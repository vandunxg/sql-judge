import { useRef, useState } from "react"
import { Plus, RefreshCw, Shield } from "lucide-react"

import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"
import { EnvironmentActionDialog } from "./environments-action-dialog"
import { useEnvironments } from "./environments-controller"
import { EnvironmentCreateDialog } from "./environments-create-dialog"
import { packLabel } from "./environments-model"
import { RuntimeCard } from "./environments-runtime-card"
import { EnvironmentsTable, type RequestAction } from "./environments-table"
import { CARD_CLASS } from "./environments-ui"

export function EnvironmentsScreen() {
  const state = useEnvironments()
  const [creating, setCreating] = useState(false)
  const openerRef = useRef<HTMLElement | null>(null)
  const { pendingAction, phase } = state

  const requestAction: RequestAction = (action, environment, opener) => {
    openerRef.current = opener
    state.requestAction(action, environment)
  }

  return (
    <main className="min-h-0 flex-1 overflow-y-auto px-4 pt-8 pb-12 sm:px-8 lg:px-10">
      <div className="mx-auto flex max-w-[1080px] flex-col gap-6">
        <header className="flex flex-wrap items-end justify-between gap-4">
          <div className="flex flex-col gap-1.5">
            <h1 className="text-[26px] leading-[1.2] font-semibold tracking-[-0.02em]">
              Environments
            </h1>
            <p className="max-w-[64ch] text-[15px] text-muted-foreground">
              Isolated practice schemas that SQL Judge creates and owns. Your
              own databases are never touched from here.
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="ghost"
              size="icon-lg"
              onClick={() => void state.refresh()}
              disabled={state.busy}
              aria-label="Refresh environments"
            >
              <RefreshCw className={cn(state.loading && "animate-spin")} />
            </Button>
            <Button
              onClick={(event) => {
                openerRef.current = event.currentTarget
                setCreating(true)
              }}
              disabled={state.busy || phase === "loading" || phase === "error"}
            >
              <Plus />
              New environment
            </Button>
          </div>
        </header>

        <RuntimeCard
          status={state.runtimeStatus}
          loading={state.loading}
          busy={state.busy}
          onStart={() => void state.startRuntime()}
          profiles={state.postgresProfiles}
          selectedProfileId={state.selectedProfileId}
          onProfileChange={(profileId) => void state.selectProfile(profileId)}
          onManageConnections={() => {
            window.location.hash = "connections"
          }}
        />

        <section
          aria-labelledby="environments-schemas-heading"
          className="flex flex-col gap-3"
        >
          <h2
            id="environments-schemas-heading"
            className="text-[15px] font-semibold"
          >
            Practice schemas
            {phase === "ready" ? (
              <span className="font-normal text-muted-foreground">
                {" "}
                · {state.environments.length}
              </span>
            ) : null}
          </h2>
          {phase === "loading" ? (
            <p
              className={cn(
                CARD_CLASS,
                "p-8 text-center text-sm text-muted-foreground"
              )}
              role="status"
            >
              Loading managed environments...
            </p>
          ) : phase === "error" ? (
            <div
              className={cn(
                CARD_CLASS,
                "flex flex-col items-center gap-3 p-8 text-center"
              )}
              role="alert"
            >
              <p className="text-sm text-err">{state.error}</p>
              <Button variant="outline" onClick={state.retry}>
                <RefreshCw />
                Retry
              </Button>
            </div>
          ) : phase === "empty" ? (
            <div
              className={cn(
                CARD_CLASS,
                "p-8 text-center text-sm text-muted-foreground"
              )}
            >
              No practice schema yet. Create one from an installed pack with New
              environment.
            </div>
          ) : (
            <EnvironmentsTable
              environments={state.environments}
              packs={state.packs}
              busy={state.busy}
              onAction={requestAction}
            />
          )}
          <p className="flex items-center gap-2 text-[13px] text-muted-foreground">
            <Shield className="size-3.5 shrink-0" aria-hidden="true" />
            Reset, clone and delete always show what will change and ask you to
            confirm. Submission history is never affected.
          </p>
          {state.message ? (
            <p
              className={cn(
                "text-sm",
                state.message.alert ? "text-err" : "text-muted-foreground"
              )}
              role={state.message.alert ? "alert" : "status"}
            >
              {state.message.text}
            </p>
          ) : null}
        </section>
      </div>

      {creating ? (
        <EnvironmentCreateDialog
          packs={state.packs}
          profiles={state.profiles}
          busy={state.busy}
          returnFocus={openerRef}
          onCancel={() => setCreating(false)}
          onCreateExternal={state.createExternalEnvironment}
        />
      ) : null}
      {pendingAction && phase === "ready" ? (
        <EnvironmentActionDialog
          key={`${pendingAction.action}:${pendingAction.environment.envId}`}
          pending={pendingAction}
          packTitle={packLabel(pendingAction.environment, state.packs)}
          takenNamespaces={state.environments.map(
            (environment) => environment.namespace
          )}
          busy={state.busy}
          returnFocus={openerRef}
          onCancel={state.cancelAction}
          onConfirm={state.confirmPendingAction}
        />
      ) : null}
    </main>
  )
}
