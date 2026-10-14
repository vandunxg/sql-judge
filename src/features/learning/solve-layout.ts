export function resizeSplit(
  value: number,
  delta: number,
  minimum: number,
  maximum: number
) {
  return Math.min(maximum, Math.max(minimum, value + delta))
}

export function resizeResultSplit(
  value: number,
  pointerDelta: number,
  minimum: number,
  maximum: number
) {
  return resizeSplit(value, -pointerDelta, minimum, maximum)
}
