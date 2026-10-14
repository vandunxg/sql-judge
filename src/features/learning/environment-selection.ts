let selectedProfileId: string | null = null
const listeners = new Set<() => void>()

export function getSelectedEnvironmentProfileId() {
  return selectedProfileId
}

export function setSelectedEnvironmentProfileId(profileId: string | null) {
  if (selectedProfileId === profileId) {
    return
  }
  selectedProfileId = profileId
  listeners.forEach((listener) => listener())
}

export function subscribeToEnvironmentProfileSelection(listener: () => void) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}
