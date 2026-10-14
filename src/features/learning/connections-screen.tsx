import { useRef, useState } from "react"

import { readConnectionProfileError } from "@/lib/connection-errors"
import { deleteConnectionProfile, saveConnectionProfile, type ConnectionProfileSummary } from "@/lib/judge-api"
import { isTauri } from "@/lib/tauri"

import { useProfileChecks } from "./connections-check"
import { EMPTY_DRAFT, draftFromProfile, saveRequestFromDraft } from "./connections-profile"
import { ConnectionsProfileForm } from "./connections-profile-form"
import { ConnectionsProfileList } from "./connections-profile-list"
import { ConnectionsDeleteDialog } from "./connections-delete-dialog"
import { isConnectionProfileEditorDisabled } from "./connections-state"
import { useConnectionProfiles } from "./connections-use-profiles"

export function ConnectionsScreen() {
  const profilesState = useConnectionProfiles()
  const { checks, testingId, test, forget } = useProfileChecks()
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [draft, setDraft] = useState(EMPTY_DRAFT)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [pendingDeletion, setPendingDeletion] = useState<ConnectionProfileSummary | null>(null)
  const deleteOpenerRef = useRef<HTMLElement | null>(null)
  const desktopAvailable = isTauri()
  const editorDisabled = isConnectionProfileEditorDisabled(desktopAvailable, busy)
  const saved = profilesState.profiles.find((profile) => profile.profileId === selectedId)

  function select(profile: ConnectionProfileSummary | null) {
    setSelectedId(profile?.profileId ?? null)
    setDraft(profile ? draftFromProfile(profile) : EMPTY_DRAFT)
    setError(null)
  }

  async function save() {
    setBusy(true)
    setError(null)
    try {
      const result = await saveConnectionProfile(saveRequestFromDraft(draft))
      profilesState.upsert(result)
      forget(result.profileId)
      select(result)
    } catch (reason) {
      setError(readConnectionProfileError(reason))
    } finally {
      setBusy(false)
    }
  }

  async function remove(profile: ConnectionProfileSummary): Promise<string | null> {
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
      return reason instanceof Error ? reason.message : "Profile could not be deleted."
    } finally {
      setBusy(false)
    }
  }

  return (
    <main className="min-h-0 flex-1 overflow-y-auto px-4 pt-8 pb-12 sm:px-10">
      <div className="mx-auto flex max-w-[1120px] flex-col gap-6">
        <header className="flex flex-col gap-1.5">
          <h1 className="text-[26px] leading-[1.2] font-semibold tracking-[-0.02em]">Connections</h1>
          <p className="max-w-[66ch] text-[15px] text-muted-foreground">
            Read-only profiles for browsing your own PostgreSQL and SQL Server databases. SQL Judge
            never writes to them, resets them or judges on them.
          </p>
        </header>
        <div className="flex flex-wrap items-start gap-5">
          <ConnectionsProfileList
            profiles={profilesState.profiles}
            phase={profilesState.phase}
            error={profilesState.error}
            selectedId={selectedId}
            checks={checks}
            disabled={editorDisabled}
            onNew={() => select(null)}
            onSelect={select}
            onRetry={() => {
              select(null)
              profilesState.retry()
            }}
          />
          <ConnectionsProfileForm
            draft={draft}
            saved={saved}
            check={saved ? checks[saved.profileId] : undefined}
            testing={saved !== undefined && testingId === saved.profileId}
            desktopAvailable={desktopAvailable}
            disabled={editorDisabled}
            error={error}
            onChange={setDraft}
            onSave={() => void save()}
            onDelete={() => {
              if (saved) {
                setPendingDeletion(saved)
              }
            }}
            onTest={() => saved && void test(saved)}
          />
        </div>
      </div>
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
