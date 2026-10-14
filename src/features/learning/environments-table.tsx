import { useRef } from "react"
import { Copy, Ellipsis, Trash2 } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import type { InstalledPack, ManagedEnvironmentSummary } from "@/lib/judge-api"
import { cn } from "@/lib/utils"
import { isDestructiveActionAvailable } from "./environment-impact"
import type { PendingAction } from "./environments-commands"
import {
  environmentBlockedReason,
  environmentStatus,
  packLabel,
} from "./environments-model"
import { CARD_CLASS, SMALL_BUTTON_CLASS, StatusBadge } from "./environments-ui"

export type RequestAction = (
  action: PendingAction["action"],
  environment: ManagedEnvironmentSummary,
  opener: HTMLElement | null
) => void

const HEAD_CLASS =
  "border-b px-4 py-2.5 text-left text-[13px] font-medium whitespace-nowrap text-muted-foreground"

export function EnvironmentsTable({
  environments,
  packs,
  busy,
  onAction,
}: {
  environments: ManagedEnvironmentSummary[]
  packs: InstalledPack[]
  busy: boolean
  onAction: RequestAction
}) {
  return (
    <div className={cn(CARD_CLASS, "overflow-hidden")}>
      <div className="overflow-x-auto">
        <table className="w-full border-collapse text-sm">
          <thead>
            <tr>
              <th scope="col" className={HEAD_CLASS}>
                Schema
              </th>
              <th scope="col" className={HEAD_CLASS}>
                Pack
              </th>
              <th scope="col" className={HEAD_CLASS}>
                Status
              </th>
              <th scope="col" className={cn(HEAD_CLASS, "w-[150px]")}>
                <span className="sr-only">Actions</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {environments.map((environment) => (
              <EnvironmentRow
                key={environment.envId}
                environment={environment}
                packTitle={packLabel(environment, packs)}
                busy={busy}
                onAction={onAction}
              />
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}

function EnvironmentRow({
  environment,
  packTitle,
  busy,
  onAction,
}: {
  environment: ManagedEnvironmentSummary
  packTitle: string
  busy: boolean
  onAction: RequestAction
}) {
  const menuTriggerRef = useRef<HTMLButtonElement>(null)
  const status = environmentStatus(environment.state)
  const blocked = environmentBlockedReason(environment, busy)
  const resetAvailable = isDestructiveActionAvailable(
    "reset",
    environment,
    busy
  )
  const deleteAvailable = isDestructiveActionAvailable(
    "delete",
    environment,
    busy
  )
  const cell = "h-[60px] border-b px-4 align-middle group-last:border-b-0"

  return (
    <tr className="group hover:bg-accent">
      <td className={cell}>
        <div className="font-mono font-medium">{environment.namespace}</div>
        {status.tone === "warn" || status.tone === "err" ? (
          <div
            className={cn(
              "text-[12.5px]",
              status.tone === "warn" ? "text-warn" : "text-err"
            )}
          >
            {environmentBlockedReason(environment, false)}
          </div>
        ) : environment.envId !== environment.namespace ? (
          <div className="text-[12.5px] text-muted-foreground">
            Environment {environment.envId}
          </div>
        ) : null}
      </td>
      <td className={cell}>
        <div>
          {packTitle}{" "}
          <span className="font-mono text-[12.5px] text-muted-foreground">
            {environment.packVersion}
          </span>
        </div>
        <div
          className="font-mono text-[12.5px] text-muted-foreground"
          title={environment.packDigest}
        >
          {environment.packDigest.slice(0, 12)}
        </div>
      </td>
      <td className={cell}>
        <StatusBadge tone={status.tone} pending={status.pending}>
          {status.label}
        </StatusBadge>
      </td>
      <td className={cn(cell, "text-right whitespace-nowrap")}>
        <span title={blocked ?? undefined} className="inline-flex">
          <Button
            variant="outline"
            className={cn(
              SMALL_BUTTON_CLASS,
              "bg-card shadow-[0_1px_2px_rgba(16,16,24,0.05)]"
            )}
            disabled={!resetAvailable}
            onClick={(event) =>
              onAction("reset", environment, event.currentTarget)
            }
            aria-label={`Reset ${environment.namespace}`}
          >
            Reset
          </Button>
        </span>{" "}
        <DropdownMenu>
          <DropdownMenuTrigger
            ref={menuTriggerRef}
            render={
              <Button
                variant="ghost"
                size="icon"
                aria-label={`More actions for ${environment.namespace}`}
              />
            }
          >
            <Ellipsis />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-44">
            <DropdownMenuItem
              disabled={blocked !== null}
              onClick={() =>
                onAction("clone", environment, menuTriggerRef.current)
              }
            >
              <Copy />
              Clone
            </DropdownMenuItem>
            <DropdownMenuItem
              variant="destructive"
              disabled={!deleteAvailable}
              onClick={() =>
                onAction("delete", environment, menuTriggerRef.current)
              }
            >
              <Trash2 />
              Delete
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </td>
    </tr>
  )
}
