import { useEffect, useState } from "react"

import {
  getManagedRuntimeStatus,
  listConnectionProfiles,
  listInstalledPacks,
  listManagedEnvironments,
  type InstalledPack,
  type ConnectionProfileSummary,
  type ManagedEnvironmentSummary,
  type ManagedRuntimeStatus,
} from "@/lib/judge-api"
import { learningLoadErrorMessage } from "./load-state"

function loadAll() {
  return Promise.all([
    getManagedRuntimeStatus(),
    listManagedEnvironments(),
    listInstalledPacks(),
    listConnectionProfiles(),
  ])
}

/** Runtime status, managed environments and installed packs, loaded together. */
export function useEnvironmentData() {
  const [runtimeStatus, setRuntimeStatus] =
    useState<ManagedRuntimeStatus | null>(null)
  const [packs, setPacks] = useState<InstalledPack[]>([])
  const [profiles, setProfiles] = useState<ConnectionProfileSummary[]>([])
  const [environments, setEnvironments] = useState<ManagedEnvironmentSummary[]>(
    []
  )
  const [busy, setBusy] = useState(true)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [reloadToken, setReloadToken] = useState(0)

  function applyLoaded([status, records, installed, connectionProfiles]: Awaited<
    ReturnType<typeof loadAll>
  >) {
    setRuntimeStatus(status)
    setEnvironments(records)
    setPacks(installed)
    setProfiles(connectionProfiles)
    setError(null)
  }

  function applyFailed() {
    setRuntimeStatus(null)
    setEnvironments([])
    setPacks([])
    setProfiles([])
    setError(learningLoadErrorMessage("environments"))
  }

  useEffect(() => {
    let active = true
    void loadAll()
      .then((loaded) => active && applyLoaded(loaded))
      .catch(() => active && applyFailed())
      .finally(() => {
        if (active) {
          setLoading(false)
          setBusy(false)
        }
      })
    return () => {
      active = false
    }
  }, [reloadToken])

  async function refresh(): Promise<boolean> {
    setLoading(true)
    setError(null)
    setBusy(true)
    try {
      applyLoaded(await loadAll())
      return true
    } catch {
      applyFailed()
      return false
    } finally {
      setLoading(false)
      setBusy(false)
    }
  }

  function retry() {
    setLoading(true)
    setError(null)
    setBusy(true)
    setReloadToken((current) => current + 1)
  }

  return {
    runtimeStatus,
    setRuntimeStatus,
    packs,
    profiles,
    environments,
    busy,
    setBusy,
    loading,
    error,
    refresh,
    retry,
  }
}
