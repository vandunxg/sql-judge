import { useRef, useState } from "react"

import { isReadOnlyVerified } from "@/lib/external-access"
import { readExternalError } from "@/lib/external-errors"
import type {
  ConnectionProfileSummary,
  ExternalConnectionInfo,
} from "@/lib/judge-api"

import {
  beginExplorerRequest,
  isCurrentExplorerRequest,
} from "./connections-state"
import { externalCommandsFor } from "./explorer-api"

export type ProfileCheck =
  | {
      ok: true
      info: ExternalConnectionInfo
      elapsedMs: number
      checkedAt: Date
    }
  | { ok: false; message: string }

export const READ_ONLY_UNVERIFIED_MESSAGE =
  "The backend did not verify a read-only connection. Metadata and SELECT actions are disabled."

const profileChecks = new Map<string, ProfileCheck>()

export function profileCheckFromInfo(
  info: ExternalConnectionInfo,
  elapsedMs: number,
  checkedAt: Date
): ProfileCheck {
  return isReadOnlyVerified(info.readOnly)
    ? { ok: true, info, elapsedMs, checkedAt }
    : { ok: false, message: READ_ONLY_UNVERIFIED_MESSAGE }
}

export async function checkProfile(
  profile: ConnectionProfileSummary
): Promise<ProfileCheck> {
  const startedAt = performance.now()
  try {
    const info = await externalCommandsFor(profile.engine).test(
      profile.profileId
    )
    return profileCheckFromInfo(
      info,
      Math.round(performance.now() - startedAt),
      new Date()
    )
  } catch (reason) {
    return { ok: false, message: readExternalError(reason) }
  }
}

export function useProfileChecks() {
  const [checks, setChecks] = useState<Record<string, ProfileCheck>>(() =>
    Object.fromEntries(profileChecks)
  )
  const [testingId, setTestingId] = useState<string | null>(null)
  const generationRef = useRef(0)

  function forget(profileId: string) {
    generationRef.current += 1
    setTestingId(null)
    profileChecks.delete(profileId)
    setChecks((current) =>
      Object.fromEntries(
        Object.entries(current).filter(([id]) => id !== profileId)
      )
    )
  }

  async function test(
    profile: ConnectionProfileSummary
  ): Promise<ProfileCheck | null> {
    const request = beginExplorerRequest(
      generationRef.current,
      profile.profileId
    )
    generationRef.current = request.generation
    setTestingId(profile.profileId)
    const check = await checkProfile(profile)
    if (
      isCurrentExplorerRequest(
        request,
        generationRef.current,
        profile.profileId
      )
    ) {
      profileChecks.set(profile.profileId, check)
      setChecks((current) => ({ ...current, [profile.profileId]: check }))
      setTestingId(null)
      return check
    }
    return null
  }

  return { checks, testingId, test, forget }
}
