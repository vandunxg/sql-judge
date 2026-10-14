import {
  cloneManagedEnvironment,
  deleteManagedEnvironment,
  resetManagedEnvironment,
  type ManagedEnvironmentSummary,
} from "@/lib/judge-api"

export type PendingAction =
  | {
      action: "reset"
      environment: ManagedEnvironmentSummary
      replacementNamespace: string
    }
  | { action: "delete"; environment: ManagedEnvironmentSummary }
  | { action: "clone"; environment: ManagedEnvironmentSummary }

export const ACTION_OUTCOME = {
  reset: {
    success: "Environment rebuilt and old schema cleaned up.",
    failure: "Environment rebuild failed closed.",
    completed: "Environment rebuild completed",
  },
  delete: {
    success: "Environment deleted.",
    failure: "Environment deletion failed closed.",
    completed: "Environment deletion completed",
  },
  clone: {
    success: "Environment cloned.",
    failure: "Environment could not be cloned.",
    completed: "Environment clone completed",
  },
} as const

/**
 * Sends the reviewed action to the native layer. Destructive calls carry the
 * exact environment + runtime instance identity and explicit confirmation;
 * Rust re-verifies ownership before touching any schema.
 */
export async function runPendingAction(
  pending: PendingAction,
  cloneNamespace: string
): Promise<void> {
  const { envId, instanceId } = pending.environment
  switch (pending.action) {
    case "reset":
      await resetManagedEnvironment({
        envId,
        instanceId,
        namespace: pending.replacementNamespace,
        confirmed: true,
      })
      return
    case "delete":
      await deleteManagedEnvironment({ envId, instanceId, confirmed: true })
      return
    case "clone":
      await cloneManagedEnvironment({
        sourceEnvId: envId,
        envId: cloneNamespace,
        namespace: cloneNamespace,
      })
  }
}
