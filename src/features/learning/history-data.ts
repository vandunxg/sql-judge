import { useEffect, useState } from "react"

import {
  listInstalledPacks,
  listSubmissionHistory,
  type InstalledPack,
  type SubmissionHistorySummary,
} from "@/lib/judge-api"

import { historyEntries, type HistoryEntry } from "./history-state"
import { learningLoadErrorMessage } from "./load-state"

export type HistoryData = {
  records: SubmissionHistorySummary[]
  packs: InstalledPack[]
  entries: HistoryEntry[]
  packsUnavailable: boolean
  /** Unix seconds when the data was loaded; the reference point for relative times. */
  loadedAt: number
}

export function useHistoryData() {
  const [data, setData] = useState<HistoryData | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [reloadToken, setReloadToken] = useState(0)

  useEffect(() => {
    let active = true
    void Promise.all([
      listSubmissionHistory(),
      listInstalledPacks().then(
        (packs) => ({ packs, packsUnavailable: false }),
        () => ({ packs: [], packsUnavailable: true })
      ),
    ])
      .then(([records, { packs, packsUnavailable }]) => {
        if (!active) return
        setData({
          records,
          packs,
          entries: historyEntries(records, packs),
          packsUnavailable,
          loadedAt: Date.now() / 1000,
        })
        setError(null)
      })
      .catch(() => {
        if (active) setError(learningLoadErrorMessage("history"))
      })
    return () => {
      active = false
    }
  }, [reloadToken])

  function retry() {
    setData(null)
    setError(null)
    setReloadToken((token) => token + 1)
  }

  return { data, error, loading: !data && !error, retry }
}
