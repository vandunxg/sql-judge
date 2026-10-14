import { useRef, useState } from "react"
import { Download, Upload } from "lucide-react"

import { AccessibleModal } from "@/components/ui/accessible-modal"
import { Button } from "@/components/ui/button"
import { exportBackup, restoreBackup, type BackupSnapshot } from "@/lib/judge-api"
import { serializeBackupForDownload } from "@/lib/backup-export"
import {
  backupProgressMessage,
  backupRestoreImpactMessage,
  backupSizeError,
  backupSuccessMessage,
  readBackupError,
} from "@/lib/backup-errors"

import { SettingRow, SettingsSection } from "./settings-rows"

const outlineButtonClass =
  "shadow-[0_1px_2px_rgba(16,16,24,0.05)] dark:shadow-[0_1px_2px_rgba(0,0,0,0.5)]"

export function SettingsBackup() {
  const inputRef = useRef<HTMLInputElement>(null)
  const restoreButtonRef = useRef<HTMLButtonElement>(null)
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState<string | null>(null)
  const [pendingFile, setPendingFile] = useState<File | null>(null)
  const locked = busy || pendingFile !== null

  function clearFileInput() {
    if (inputRef.current) {
      inputRef.current.value = ""
    }
  }

  async function handleExport() {
    setBusy(true)
    setMessage(backupProgressMessage("export"))
    try {
      const snapshot = await exportBackup()
      const blob = new Blob([serializeBackupForDownload(snapshot)], { type: "application/json" })
      const url = URL.createObjectURL(blob)
      const link = document.createElement("a")
      link.href = url
      link.download = "sql-judge-backup.json"
      link.click()
      URL.revokeObjectURL(url)
      setMessage(backupSuccessMessage("export"))
    } catch (reason) {
      setMessage(readBackupError(reason, "export"))
    } finally {
      setBusy(false)
    }
  }

  function handleFile(file: File | undefined) {
    if (!file) {
      return
    }
    const sizeError = backupSizeError(file.size)
    setMessage(sizeError)
    setPendingFile(sizeError ? null : file)
    clearFileInput()
  }

  async function confirmRestore() {
    if (!pendingFile) {
      return
    }
    setBusy(true)
    setMessage(backupProgressMessage("restore"))
    try {
      const snapshot = JSON.parse(await pendingFile.text()) as BackupSnapshot
      await restoreBackup(snapshot)
      setMessage(backupSuccessMessage("restore"))
    } catch (reason) {
      setMessage(readBackupError(reason, "restore"))
    } finally {
      setPendingFile(null)
      setBusy(false)
      clearFileInput()
    }
  }

  return (
    <SettingsSection id="backup" title="Backup and data">
      <div aria-hidden={pendingFile ? true : undefined} inert={pendingFile ? true : undefined}>
        <SettingRow
          title="Export a backup"
          description="Environment metadata and submission history in one file. Passwords and keychain entries are left out."
        >
          <Button variant="outline" className={outlineButtonClass} onClick={() => void handleExport()} disabled={locked}>
            <Download />
            Export
          </Button>
        </SettingRow>
        <SettingRow
          title="Restore from a backup"
          description="You'll confirm what gets replaced before anything changes. Files over 4 MiB are rejected."
        >
          <input
            ref={inputRef}
            type="file"
            accept="application/json,.json"
            className="sr-only"
            tabIndex={-1}
            aria-hidden="true"
            onChange={(event) => handleFile(event.target.files?.[0])}
            disabled={locked}
          />
          <Button
            ref={restoreButtonRef}
            variant="outline"
            className={outlineButtonClass}
            onClick={() => inputRef.current?.click()}
            disabled={locked}
          >
            <Upload />
            Restore
          </Button>
        </SettingRow>
      </div>
      {message ? (
        <p className="border-t border-border px-[22px] py-3 text-[13px] text-muted-foreground" role="status">
          {message}
        </p>
      ) : null}
      {pendingFile ? (
        <AccessibleModal
          open
          title="Confirm backup restore"
          description={backupRestoreImpactMessage()}
          restoreFocusRef={restoreButtonRef}
          onCancel={() => setPendingFile(null)}
          busy={busy}
        >
          <p className="mt-2 truncate font-mono text-xs" title={pendingFile.name}>
            {pendingFile.name}
          </p>
          <div className="mt-3 flex justify-end gap-2">
            <Button variant="outline" size="sm" onClick={() => setPendingFile(null)} disabled={busy}>
              Cancel
            </Button>
            <Button variant="destructive" size="sm" onClick={() => void confirmRestore()} disabled={busy}>
              Confirm restore
            </Button>
          </div>
        </AccessibleModal>
      ) : null}
    </SettingsSection>
  )
}
