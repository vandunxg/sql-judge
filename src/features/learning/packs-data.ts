import { useEffect, useState } from "react"

import {
  listInstalledPacks,
  listManagedEnvironments,
  listSubmissionHistory,
  type InstalledPack,
  type ManagedEnvironmentSummary,
  type SubmissionHistorySummary,
} from "@/lib/judge-api"

import { learningLoadErrorMessage, learningLoadPhase } from "./load-state"

type PackExtras = {
  environments: ManagedEnvironmentSummary[] | null
  history: SubmissionHistorySummary[] | null
}

export function usePacksData() {
  const [packs, setPacks] = useState<InstalledPack[]>([])
  const [catalogLoading, setCatalogLoading] = useState(true)
  const [catalogError, setCatalogError] = useState<string | null>(null)
  const [catalogToken, setCatalogToken] = useState(0)
  const [extras, setExtras] = useState<PackExtras>({
    environments: null,
    history: null,
  })
  const [extrasLoading, setExtrasLoading] = useState(true)
  const [extrasToken, setExtrasToken] = useState(0)

  useEffect(() => {
    let active = true
    void listInstalledPacks()
      .then((installedPacks) => {
        if (active) setPacks(installedPacks)
      })
      .catch(() => {
        if (active) {
          setPacks([])
          setCatalogError(learningLoadErrorMessage("catalog"))
        }
      })
      .finally(() => {
        if (active) setCatalogLoading(false)
      })
    return () => {
      active = false
    }
  }, [catalogToken])

  useEffect(() => {
    let active = true
    void Promise.allSettled([
      listManagedEnvironments(),
      listSubmissionHistory(),
    ]).then(([environments, history]) => {
      if (!active) return
      setExtras({
        environments:
          environments.status === "fulfilled" ? environments.value : null,
        history: history.status === "fulfilled" ? history.value : null,
      })
      setExtrasLoading(false)
    })
    return () => {
      active = false
    }
  }, [extrasToken])

  function retryCatalog() {
    setCatalogLoading(true)
    setCatalogError(null)
    setCatalogToken((current) => current + 1)
  }

  function retryExtras() {
    setExtrasLoading(true)
    setExtrasToken((current) => current + 1)
  }

  /** Returns the refreshed catalog, or null when the refresh failed. */
  async function refreshAfterImport() {
    setCatalogLoading(true)
    setCatalogError(null)
    try {
      const installedPacks = await listInstalledPacks()
      setPacks(installedPacks)
      retryExtras()
      return installedPacks
    } catch {
      setCatalogError(learningLoadErrorMessage("catalog"))
      return null
    } finally {
      setCatalogLoading(false)
    }
  }

  return {
    packs,
    catalogError,
    catalogPhase: learningLoadPhase(catalogLoading, catalogError, packs.length > 0),
    environments: extras.environments,
    history: extras.history,
    extrasFailed:
      !extrasLoading &&
      (extras.environments === null || extras.history === null),
    retryCatalog,
    retryExtras,
    refreshAfterImport,
  }
}
