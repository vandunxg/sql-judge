import { useRef, useState } from "react"

import { isReadOnlyVerified } from "@/lib/external-access"
import { readExternalError } from "@/lib/external-errors"
import type { ConnectionProfileSummary, ExternalConnectionInfo } from "@/lib/judge-api"

import { beginExplorerRequest, isCurrentExplorerRequest } from "./connections-state"
import { externalCommandsFor } from "./explorer-api"

export type ProfileCheck =
  | { ok: true; info: ExternalConnectionInfo; elapsedMs: number; checkedAt: Date }
  | { ok: false; message: string }

export const READ_ONLY_UNVERIFIED_MESSAGE =
  "The backend did not verify a read-only connection. Metadata and SELECT actions are disabled."

export function profileCheckFromInfo(
  info: ExternalConnectionInfo,
  elapsedMs: number,
  checkedAt: Date
): ProfileCheck {
  return isReadOnlyVerified(info.readOnly)
    ? { ok: true, info, elapsedMs, checkedAt }
    : { ok: false, message: READ_ONLY_UNVERIFIED_MESSAGE }
}

export async function checkProfile(profile: ConnectionProfileSummary): Promise<ProfileCheck> {
  const startedAt = performance.now()
  try {
    const info = await externalCommandsFor(profile.engine).test(profile.profileId)
    return profileCheckFromInfo(info, Math.round(performance.now() - startedAt), new Date())
  } catch (reason) {
    return { ok: false, message: readExternalError(reason) }
  }
}

export function useProfileChecks() {
  const [checks, setChecks] = useState<Record<string, ProfileCheck>>({})
  const [testingId, setTestingId] = useState<string | null>(null)
  const generationRef = useRef(0)

  function forget(profileId: string) {
    generationRef.current += 1
    setTestingId(null)
    setChecks((current) =>
      Object.fromEntries(Object.entries(current).filter(([id]) => id !== profileId))
    )
  }

  async function test(profile: ConnectionProfileSummary) {
    const request = beginExplorerRequest(generationRef.current, profile.profileId)
    generationRef.current = request.generation
    setTestingId(profile.profileId)
    const check = await checkProfile(profile)
    if (isCurrentExplorerRequest(request, generationRef.current, profile.profileId)) {
      setChecks((current) => ({ ...current, [profile.profileId]: check }))
      setTestingId(null)
    }
  }

  return { checks, testingId, test, forget }
}
