import { useEffect, useRef, useState } from "react"

import { listConnectionProfiles, type ConnectionProfileSummary } from "@/lib/judge-api"

import { shouldApplyProfileLoad } from "./connections-state"
import { learningLoadErrorMessage, learningLoadPhase } from "./load-state"

export function useConnectionProfiles() {
  const [profiles, setProfiles] = useState<ConnectionProfileSummary[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [reloadToken, setReloadToken] = useState(0)
  const mutationGenerationRef = useRef(0)

  useEffect(() => {
    let active = true
    const loadGeneration = mutationGenerationRef.current
    const applies = () =>
      active && shouldApplyProfileLoad(loadGeneration, mutationGenerationRef.current)
    void listConnectionProfiles()
      .then((loaded) => {
        if (applies()) {
          setProfiles(loaded)
        }
      })
      .catch(() => {
        if (applies()) {
          setProfiles([])
          setError(learningLoadErrorMessage("profiles"))
        }
      })
      .finally(() => {
        if (active) {
          setLoading(false)
        }
      })
    return () => {
      active = false
    }
  }, [reloadToken])

  function retry() {
    setLoading(true)
    setError(null)
    setReloadToken((current) => current + 1)
  }

  function upsert(saved: ConnectionProfileSummary) {
    mutationGenerationRef.current += 1
    setProfiles((current) =>
      [...current.filter((profile) => profile.profileId !== saved.profileId), saved].sort(
        (left, right) => left.profileId.localeCompare(right.profileId)
      )
    )
  }

  function remove(profileId: string) {
    mutationGenerationRef.current += 1
    setProfiles((current) => current.filter((profile) => profile.profileId !== profileId))
  }

  return {
    profiles,
    error,
    phase: learningLoadPhase(loading, error, profiles.length > 0),
    retry,
    upsert,
    remove,
  }
}
