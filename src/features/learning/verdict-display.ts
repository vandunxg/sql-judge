import type { JudgeRunResult, JudgeVerdict } from "@/lib/judge-api"

type VerdictSource = Pick<JudgeRunResult, "status" | "verdict">
export type VerdictTone = "ok" | "err" | "warn"

const verdicts: Record<JudgeVerdict, { label: string; tone: VerdictTone }> = {
  Accepted: { label: "Accepted", tone: "ok" },
  WrongAnswer: { label: "Wrong answer", tone: "err" },
  SyntaxError: { label: "Syntax error", tone: "err" },
  RuntimeError: { label: "Runtime error", tone: "err" },
  PolicyViolation: { label: "Policy violation", tone: "warn" },
  TimeLimitExceeded: { label: "Time limit exceeded", tone: "warn" },
  ResultLimitExceeded: { label: "Result limit exceeded", tone: "warn" },
  Cancelled: { label: "Cancelled", tone: "warn" },
  JudgeError: { label: "Judge error", tone: "warn" },
}

export function verdictLabel(result: VerdictSource) {
  return result.verdict ? verdicts[result.verdict].label : "Runner unavailable"
}

export function verdictTone(result: VerdictSource): VerdictTone {
  return result.verdict ? verdicts[result.verdict].tone : "warn"
}

export function isJudgeVerdict(value: string): value is JudgeVerdict {
  return Object.hasOwn(verdicts, value)
}
