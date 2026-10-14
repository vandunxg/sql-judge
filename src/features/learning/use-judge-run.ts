import { useCallback, useEffect, useRef, useState } from "react"

import { cancelJudge, runJudge, type JudgeRunResult } from "@/lib/judge-api"

import type { LearningProblem } from "./catalog"
import {
  beginJudgeRequest,
  isCurrentJudgeRequest,
  type JudgeRequest,
} from "./judge-state"
import { stopVisibleAfter } from "./practice-workspace-state"

export type JudgeMode = "run" | "submit"

export function useJudgeRun() {
  const [result, setResult] = useState<JudgeRunResult | null>(null)
  const [resultMode, setResultMode] = useState<JudgeMode>("run")
  const [activeMode, setActiveMode] = useState<JudgeMode | null>(null)
  const [elapsedBusyMs, setElapsedBusyMs] = useState(0)
  const selectedProblemIdRef = useRef("")
  const judgeGenerationRef = useRef(0)
  const activeJudgeRequestRef = useRef<JudgeRequest | null>(null)
  const activeRequestIdRef = useRef<string | null>(null)
  const busy = activeMode !== null

  const invalidate = useCallback(() => {
    judgeGenerationRef.current += 1
    activeJudgeRequestRef.current = null
    activeRequestIdRef.current = null
    setActiveMode(null)
    setResult(null)
  }, [])

  const trackProblem = useCallback(
    (problemId: string) => {
      if (selectedProblemIdRef.current !== problemId) {
        invalidate()
        selectedProblemIdRef.current = problemId
      }
    },
    [invalidate]
  )

  const trackedProblemId = useCallback(() => selectedProblemIdRef.current, [])

  useEffect(() => {
    if (!busy) {
      return
    }
    const startedAt = Date.now()
    const timer = window.setInterval(
      () => setElapsedBusyMs(Date.now() - startedAt),
      100
    )
    return () => window.clearInterval(timer)
  }, [busy])

  async function run(problem: LearningProblem, sql: string, mode: JudgeMode) {
    const request = beginJudgeRequest(judgeGenerationRef.current, problem.id)
    const isCurrent = () =>
      isCurrentJudgeRequest(
        request,
        judgeGenerationRef.current,
        selectedProblemIdRef.current
      )
    judgeGenerationRef.current = request.generation
    const requestId = crypto.randomUUID()
    activeJudgeRequestRef.current = request
    activeRequestIdRef.current = requestId
    setElapsedBusyMs(0)
    setActiveMode(mode)
    setResultMode(mode)
    setResult(null)
    try {
      const nextResult = await runJudge({
        problemId: problem.problemId,
        packId: problem.packId,
        packVersion: problem.packVersion,
        packDigest: problem.packDigest,
        sql,
        mode,
        requestId,
      })
      if (isCurrent()) {
        setResult(nextResult)
      }
    } catch {
      if (isCurrent()) {
        setResult({
          status: "error",
          durationMs: 0,
          message:
            "The native judge returned an unavailable or malformed response.",
        })
      }
    } finally {
      if (isCurrent()) {
        activeJudgeRequestRef.current = null
        activeRequestIdRef.current = null
        setActiveMode(null)
      }
    }
  }

  async function cancel() {
    const requestId = activeRequestIdRef.current
    if (activeJudgeRequestRef.current && requestId) {
      await cancelJudge(requestId).catch(() => undefined)
    }
  }

  return {
    result,
    resultMode,
    activeMode,
    busy,
    canStop: busy && stopVisibleAfter(elapsedBusyMs),
    invalidate,
    trackProblem,
    trackedProblemId,
    run,
    cancel,
  }
}
