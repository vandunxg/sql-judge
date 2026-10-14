import { useRef, useState } from "react"
import { Plus } from "lucide-react"

import { AccessibleModal } from "@/components/ui/accessible-modal"
import { Button } from "@/components/ui/button"
import { readConnectionProfileError } from "@/lib/connection-errors"
import {
  deleteConnectionProfile,
  saveConnectionProfile,
  type ConnectionProfileSummary,
} from "@/lib/judge-api"
import { isTauri } from "@/lib/tauri"

import { useProfileChecks } from "./connections-check"
import {
  EMPTY_DRAFT,
  draftFromProfile,
  saveRequestFromDraft,
} from "./connections-profile"
import { ConnectionsProfileForm } from "./connections-profile-form"
import { ConnectionsProfileList } from "./connections-profile-list"
import { ConnectionsDeleteDialog } from "./connections-delete-dialog"
import { isConnectionProfileEditorDisabled } from "./connections-state"
import { useConnectionProfiles } from "./connections-use-profiles"

export function ConnectionsScreen() {
  const profilesState = useConnectionProfiles()
  const { checks, testingId, test, forget } = useProfileChecks()
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [createOpen, setCreateOpen] = useState(false)
  const [draft, setDraft] = useState(EMPTY_DRAFT)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [pendingDeletion, setPendingDeletion] =
    useState<ConnectionProfileSummary | null>(null)
  const deleteOpenerRef = useRef<HTMLElement | null>(null)
  const desktopAvailable = isTauri()
  const editorDisabled = isConnectionProfileEditorDisabled(
    desktopAvailable,
    busy
  )
  const saved = profilesState.profiles.find(
    (profile) => profile.profileId === selectedId
  )

  function select(profile: ConnectionProfileSummary | null) {
    setSelectedId(profile?.profileId ?? null)
    setDraft(profile ? draftFromProfile(profile) : EMPTY_DRAFT)
    setError(null)
  }

  function startCreate() {
    select(null)
    setCreateOpen(true)
  }

  async function save() {
    setBusy(true)
    setError(null)
    try {
      const result = await saveConnectionProfile(saveRequestFromDraft(draft))
      profilesState.upsert(result)
      forget(result.profileId)
      setCreateOpen(false)
      select(result)
    } catch (reason) {
      setError(readConnectionProfileError(reason))
    } finally {
      setBusy(false)
    }
  }

  async function remove(
    profile: ConnectionProfileSummary
  ): Promise<string | null> {
    setBusy(true)
    setError(null)
    try {
      await deleteConnectionProfile(profile.profileId)
      profilesState.remove(profile.profileId)
      forget(profile.profileId)
      select(null)
      setPendingDeletion(null)
      return null
    } catch (reason) {
      return reason instanceof Error
        ? reason.message
        : "Profile could not be deleted."
    } finally {
      setBusy(false)
    }
  }

  return (
    <main className="min-h-0 flex-1 overflow-y-auto px-4 pt-8 pb-12 sm:px-10">
      <div className="mx-auto flex max-w-[1120px] flex-col gap-6">
        <header className="flex flex-wrap items-start justify-between gap-x-6 gap-y-3">
          <div className="flex min-w-0 flex-1 flex-col gap-1.5">
            <h1 className="text-[26px] leading-[1.2] font-semibold tracking-[-0.02em]">
              Connections
            </h1>
            <p className="max-w-[66ch] text-[15px] text-muted-foreground">
              Read-only profiles for browsing your own PostgreSQL and SQL Server
              databases. SQL Judge never writes to them, resets them or judges
              on them.
            </p>
          </div>
          <Button
            variant="outline"
            className="shrink-0 bg-card shadow-xs"
            onClick={startCreate}
            disabled={editorDisabled}
          >
            <Plus /> New profile
          </Button>
        </header>
        <div className="flex flex-wrap items-start gap-5">
          <ConnectionsProfileList
            profiles={profilesState.profiles}
            phase={profilesState.phase}
            error={profilesState.error}
            selectedId={selectedId}
            checks={checks}
            onSelect={select}
            onRetry={() => {
              select(null)
              profilesState.retry()
            }}
          />
          {saved ? (
            <ConnectionsProfileForm
              draft={draft}
              saved={saved}
              check={checks[saved.profileId]}
              testing={testingId === saved.profileId}
              desktopAvailable={desktopAvailable}
              disabled={editorDisabled}
              error={error}
              onChange={setDraft}
              onSave={() => void save()}
              onDelete={() => setPendingDeletion(saved)}
              onTest={() => void test(saved)}
            />
          ) : null}
        </div>
      </div>
      <AccessibleModal
        open={createOpen}
        title="New connection profile"
        description="Add a read-only connection profile for browsing your database."
        busy={busy}
        onCancel={() => setCreateOpen(false)}
        role="dialog"
        headerClassName="border-b border-border px-6 py-5"
        className="max-h-[calc(100vh-2rem)] max-w-3xl overflow-y-auto border-border bg-card p-0"
      >
        <ConnectionsProfileForm
          draft={draft}
          saved={undefined}
          check={undefined}
          testing={false}
          desktopAvailable={desktopAvailable}
          disabled={editorDisabled}
          error={error}
          onChange={setDraft}
          onSave={() => void save()}
          onDelete={() => undefined}
          onTest={() => undefined}
          onCancel={() => setCreateOpen(false)}
          modal
        />
      </AccessibleModal>
      {pendingDeletion ? (
        <ConnectionsDeleteDialog
          profile={pendingDeletion}
          returnFocus={deleteOpenerRef}
          onCancel={() => setPendingDeletion(null)}
          onDelete={() => remove(pendingDeletion)}
        />
      ) : null}
    </main>
  )
}
