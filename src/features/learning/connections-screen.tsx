import { useRef, useState } from "react"
import { Plus } from "lucide-react"
import { toast } from "sonner"

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
  const [profileOpen, setProfileOpen] = useState(false)
  const [draft, setDraft] = useState(EMPTY_DRAFT)
  const [busy, setBusy] = useState(false)
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
  }

  function startCreate() {
    select(null)
    setCreateOpen(true)
  }

  function openProfile(profile: ConnectionProfileSummary) {
    select(profile)
    setProfileOpen(true)
  }

  function connectionChanged(profile: ConnectionProfileSummary | undefined) {
    if (!profile) {
      return true
    }
    return (
      draft.profileId !== profile.profileId ||
      draft.engine !== profile.engine ||
      draft.host !== profile.host ||
      draft.port !== profile.port ||
      draft.database !== profile.database ||
      draft.username !== profile.username ||
      draft.tlsMode !== profile.tlsMode ||
      Boolean(draft.secret?.trim())
    )
  }

  async function save() {
    setBusy(true)
    try {
      const result = await saveConnectionProfile(saveRequestFromDraft(draft))
      profilesState.upsert(result)
      if (connectionChanged(saved)) {
        forget(result.profileId)
      }
      setCreateOpen(false)
      setProfileOpen(false)
      select(result)
    } catch (reason) {
      toast.error(readConnectionProfileError(reason))
    } finally {
      setBusy(false)
    }
  }

  async function testProfile(profile: ConnectionProfileSummary) {
    const toastId = toast.loading(
      "Testing the connection and verifying read-only access..."
    )
    const check = await test(profile)
    if (!check) {
      toast.dismiss(toastId)
      return
    }
    if (check.ok) {
      toast.success(
        `Connected in ${check.elapsedMs} ms · read-only session confirmed`,
        { id: toastId }
      )
    } else {
      toast.error(check.message, { id: toastId })
    }
  }

  async function remove(
    profile: ConnectionProfileSummary
  ): Promise<string | null> {
    setBusy(true)
    try {
      await deleteConnectionProfile(profile.profileId)
      profilesState.remove(profile.profileId)
      forget(profile.profileId)
      select(null)
      setPendingDeletion(null)
      setProfileOpen(false)
      return null
    } catch (reason) {
      toast.error(
        reason instanceof Error
          ? reason.message
          : "Profile could not be deleted."
      )
      return null
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
            onSelect={openProfile}
            onRetry={() => {
              select(null)
              profilesState.retry()
            }}
          />
        </div>
      </div>
      <AccessibleModal
        open={createOpen || profileOpen}
        title={saved ? saved.displayName : "New connection profile"}
        description={
          saved
            ? "Review or update this read-only connection profile."
            : "Add a read-only connection profile for browsing your database."
        }
        busy={busy}
        onCancel={() => {
          setCreateOpen(false)
          setProfileOpen(false)
        }}
        role="dialog"
        headerClassName="border-b border-border px-6 py-5"
        showCloseButton
        closeLabel="Close connection profile"
        className="max-h-[calc(100vh-2rem)] max-w-3xl overflow-y-auto border-border bg-card p-0"
      >
        <ConnectionsProfileForm
          draft={draft}
          saved={saved}
          testing={saved !== undefined && testingId === saved.profileId}
          disabled={editorDisabled}
          onChange={setDraft}
          onSave={() => void save()}
          onDelete={() => {
            if (saved) {
              setPendingDeletion(saved)
            }
          }}
          onTest={() => saved && void testProfile(saved)}
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
