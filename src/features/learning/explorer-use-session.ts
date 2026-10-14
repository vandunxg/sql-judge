import { useRef, useState } from "react"

import { isReadOnlyVerified } from "@/lib/external-access"
import { readExternalError } from "@/lib/external-errors"
import type {
  ConnectionProfileSummary,
  ExternalConnectionInfo,
  ExternalMetadata,
  ExternalQueryResult,
} from "@/lib/judge-api"

import { checkProfile } from "./connections-check"
import { SAMPLE_QUERIES } from "./connections-profile"
import { beginExplorerRequest, clearExplorerOutput, isCurrentExplorerRequest } from "./connections-state"
import { EXPLORER_LIMITS, externalCommandsFor } from "./explorer-api"

export type TimedResult = { result: ExternalQueryResult; elapsedMs: number }
export type ExplorerStep = "connect" | "metadata" | "run"
type Output = { metadata: ExternalMetadata | null; queryResult: TimedResult | null }

// One session per profile: the screen remounts this hook (keyed by profile id) on every switch.
export function useExplorerSession(profile: ConnectionProfileSummary) {
  const [info, setInfo] = useState<ExternalConnectionInfo | null>(null)
  const [output, setOutput] = useState<Output>({ metadata: null, queryResult: null })
  const [busy, setBusy] = useState<ExplorerStep | null>(null)
  const [failure, setFailure] = useState<{ step: ExplorerStep; message: string } | null>(null)
  const [query, setQuery] = useState<string>(SAMPLE_QUERIES[profile.engine])
  const generationRef = useRef(0)
  const commands = externalCommandsFor(profile.engine)
  const verified = isReadOnlyVerified(info?.readOnly)

  async function guarded<T>(step: ExplorerStep, work: () => Promise<T>): Promise<T | null> {
    const request = beginExplorerRequest(generationRef.current, profile.profileId)
    generationRef.current = request.generation
    const isCurrent = () => isCurrentExplorerRequest(request, generationRef.current, profile.profileId)
    setBusy(step)
    setFailure(null)
    try {
      const value = await work()
      return isCurrent() ? value : null
    } catch (reason) {
      if (isCurrent()) {
        setFailure({ step, message: readExternalError(reason) })
      }
      return null
    } finally {
      if (isCurrent()) {
        setBusy(null)
      }
    }
  }

  async function fetchMetadata() {
    setOutput((current) => ({ ...current, metadata: null }))
    const metadata = await guarded("metadata", () => commands.metadata(profile.profileId))
    if (metadata) {
      setOutput((current) => ({ ...current, metadata }))
    }
  }

  async function connect() {
    setInfo(null)
    setOutput(clearExplorerOutput)
    const check = await guarded("connect", async () => {
      const result = await checkProfile(profile)
      if (!result.ok) {
        throw new Error(result.message)
      }
      return result
    })
    if (check?.ok) {
      setInfo(check.info)
      await fetchMetadata()
    }
  }

  async function loadMetadata() {
    if (!verified) {
      setFailure({ step: "metadata", message: "Metadata is unavailable until the backend verifies read-only access." })
      return
    }
    await fetchMetadata()
  }

  async function run() {
    if (!verified) {
      setFailure({ step: "run", message: "SELECT is unavailable until the backend verifies read-only access." })
      return
    }
    if (busy || !query.trim()) {
      return
    }
    setOutput((current) => ({ ...current, queryResult: null }))
    const startedAt = performance.now()
    const result = await guarded("run", () =>
      commands.select({ profileId: profile.profileId, sql: query, ...EXPLORER_LIMITS })
    )
    if (result) {
      const elapsedMs = Math.round(performance.now() - startedAt)
      setOutput((current) => ({ ...current, queryResult: { result, elapsedMs } }))
    }
  }

  return { info, verified, ...output, busy, failure, query, setQuery, connect, loadMetadata, run }
}

export type ExplorerSession = ReturnType<typeof useExplorerSession>
