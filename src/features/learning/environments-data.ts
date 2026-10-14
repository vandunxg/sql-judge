import { useEffect, useRef, useState } from "react"

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
import { isCurrentEnvironmentLoad } from "./environments-model"

function loadAll(profileId: string | null) {
  return Promise.all([
    getManagedRuntimeStatus(),
    listManagedEnvironments(profileId),
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
  const loadGeneration = useRef(0)

  function applyLoaded([
    status,
    records,
    installed,
    connectionProfiles,
  ]: Awaited<ReturnType<typeof loadAll>>) {
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
    const loadId = ++loadGeneration.current
    void loadAll(null)
      .then(
        (loaded) =>
          active &&
          isCurrentEnvironmentLoad(loadId, loadGeneration.current) &&
          applyLoaded(loaded)
      )
      .catch(
        () =>
          active &&
          isCurrentEnvironmentLoad(loadId, loadGeneration.current) &&
          applyFailed()
      )
      .finally(() => {
        if (
          active &&
          isCurrentEnvironmentLoad(loadId, loadGeneration.current)
        ) {
          setLoading(false)
          setBusy(false)
        }
      })
    return () => {
      active = false
    }
  }, [reloadToken])

  async function refresh(profileId: string | null = null): Promise<boolean> {
    const loadId = ++loadGeneration.current
    setLoading(true)
    setError(null)
    setBusy(true)
    try {
      let loaded = await loadAll(profileId)
      if (
        profileId &&
        !loaded[3].some(
          (profile) =>
            profile.profileId === profileId && profile.engine === "postgresql"
        )
      ) {
        loaded = await loadAll(null)
      }
      if (isCurrentEnvironmentLoad(loadId, loadGeneration.current)) {
        applyLoaded(loaded)
      }
      return true
    } catch {
      if (isCurrentEnvironmentLoad(loadId, loadGeneration.current)) {
        applyFailed()
      }
      return false
    } finally {
      if (isCurrentEnvironmentLoad(loadId, loadGeneration.current)) {
        setLoading(false)
        setBusy(false)
      }
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
