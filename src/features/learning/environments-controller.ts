import { useState } from "react"

import {
  createManagedEnvironment,
  createExternalSandbox,
  startManagedRuntime,
  type InstalledPack,
  type ManagedEnvironmentSummary,
} from "@/lib/judge-api"
import {
  ACTION_OUTCOME,
  runPendingAction,
  type PendingAction,
} from "./environments-commands"
import { useEnvironmentData } from "./environments-data"
import { namespaceError, uniqueNamespace } from "./environments-model"
import { learningLoadPhase } from "./load-state"

export type EnvironmentMessage = { text: string; alert: boolean }

export function useEnvironments() {
  const data = useEnvironmentData()
  const { environments, setBusy } = data
  const [message, setMessage] = useState<EnvironmentMessage | null>(null)
  const [pendingAction, setPendingAction] = useState<PendingAction | null>(null)

  async function refresh(): Promise<boolean> {
    const refreshed = await data.refresh()
    if (refreshed) {
      setMessage(null)
    } else {
      setPendingAction(null)
      setMessage({
        text: "Managed environments could not be loaded.",
        alert: true,
      })
    }
    return refreshed
  }

  function retry() {
    setMessage(null)
    data.retry()
  }

  async function startRuntime() {
    setBusy(true)
    try {
      data.setRuntimeStatus(await startManagedRuntime())
      setMessage({ text: "Managed PostgreSQL is ready.", alert: false })
    } catch {
      setMessage({
        text: "The managed PostgreSQL runtime could not be started.",
        alert: true,
      })
    } finally {
      setBusy(false)
    }
  }

  async function refreshAfter(success: string, completed: string) {
    if (await refresh()) {
      setMessage({ text: success, alert: false })
    } else {
      setMessage({
        text: `${completed}, but the environment list could not be refreshed. Retry to verify its state.`,
        alert: true,
      })
    }
  }

  /** Returns an error for the create dialog, or null on success. */
  async function createEnvironment(
    pack: InstalledPack | null,
    rawNamespace: string
  ): Promise<string | null> {
    const namespace = rawNamespace.trim()
    if (!pack) {
      return "Import the managed pack before creating an environment."
    }
    const invalid = namespaceError(namespace, environments)
    if (invalid) {
      return invalid
    }
    setBusy(true)
    try {
      await createManagedEnvironment({
        envId: namespace,
        packId: pack.packId,
        packVersion: pack.version,
        packDigest: pack.digest,
        namespace,
      })
    } catch {
      setBusy(false)
      return "Practice environment could not be provisioned."
    }
    await refreshAfter(
      "Practice environment is ready.",
      "Practice environment was created"
    )
    return null
  }

  async function createExternalEnvironment(
    pack: InstalledPack | null,
    envId: string,
    profileId: string,
    confirmed: boolean
  ): Promise<string | null> {
    if (!pack) {
      return "Import the managed pack before creating an environment."
    }
    if (!profileId) {
      return "Select a verified PostgreSQL profile."
    }
    if (!confirmed) {
      return "Confirm the external sandbox impact before creating it."
    }
    const normalizedEnvId = envId.trim()
    if (!normalizedEnvId) {
      return "Enter an environment ID."
    }
    setBusy(true)
    try {
      await createExternalSandbox({
        envId: normalizedEnvId,
        profileId,
        packId: pack.packId,
        packVersion: pack.version,
        packDigest: pack.digest,
        confirmed: true,
      })
    } catch {
      setBusy(false)
      return "The external sandbox could not be created. Verify the target and try again."
    }
    await refreshAfter(
      "External practice sandbox is ready.",
      "External practice sandbox was created"
    )
    return null
  }

  function requestAction(
    action: PendingAction["action"],
    environment: ManagedEnvironmentSummary
  ) {
    const taken = environments.map((candidate) => candidate.namespace)
    setPendingAction(
      action === "reset"
        ? {
            action,
            environment,
            replacementNamespace: uniqueNamespace(
              environment.namespace,
              "r",
              taken
            ),
          }
        : { action, environment }
    )
  }

  /** Clone passes its target namespace; returns a validation error or null. */
  async function confirmPendingAction(
    cloneNamespace = ""
  ): Promise<string | null> {
    if (!pendingAction) {
      return null
    }
    const target = cloneNamespace.trim()
    const invalid =
      pendingAction.action === "clone"
        ? namespaceError(target, environments)
        : null
    if (invalid) {
      return invalid
    }
    const outcome = ACTION_OUTCOME[pendingAction.action]
    setBusy(true)
    try {
      await runPendingAction(pendingAction, target)
      setPendingAction(null)
      await refreshAfter(outcome.success, outcome.completed)
    } catch {
      setPendingAction(null)
      setMessage({ text: outcome.failure, alert: true })
      setBusy(false)
    }
    return null
  }

  return {
    ...data,
    phase: learningLoadPhase(data.loading, data.error, environments.length > 0),
    message,
    pendingAction,
    refresh,
    retry,
    startRuntime,
    createEnvironment,
    createExternalEnvironment,
    requestAction,
    cancelAction: () => setPendingAction(null),
    confirmPendingAction,
  }
}
