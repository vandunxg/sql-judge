import { useEffect, useMemo, useRef, useState } from "react"

import {
  cancelJudge,
  listInstalledPacks,
  listManagedEnvironments,
  listSubmissionHistory,
  runJudge,
  type SubmissionHistorySummary,
  type ManagedEnvironmentSummary,
  type JudgeRunResult,
  type InstalledPack,
} from "@/lib/judge-api"
import {
  catalogProblems,
  canJudgeProblem,
  type LearningProblem,
} from "./catalog"
import { createDraftStore } from "./draft-store"
import {
  beginJudgeRequest,
  isCurrentJudgeRequest,
  type JudgeRequest,
} from "./judge-state"
import { learningLoadErrorMessage } from "./load-state"
import { stopVisibleAfter } from "./practice-workspace-state"
import { submissionsOnly } from "./problem-progress"
import { ProblemsList } from "./problems-list"
import { adjacentProblems, environmentForProblem } from "./solve-state"
import { SolveView } from "./solve-view"
import { sqlCompletionItems } from "./sql-completions"

const MAX_SQL_LENGTH = 65_536

// Drafts outlive the workspace so leaving Solve for the list and coming back keeps them (AC-002).
const sessionDrafts = createDraftStore(MAX_SQL_LENGTH)

const missingProblem: LearningProblem = {
  id: "",
  problemId: "",
  title: "Problem not found",
  difficulty: "easy",
  dialect: "postgresql",
  topics: [],
  statement: "",
  packId: "",
  packVersion: "",
  packDigest: "",
  schema: [],
  sampleRows: [],
  starterSql: "",
  outputColumns: [],
  rowOrder: "unordered",
  timeLimitMs: 0,
  fixtureCount: 0,
  sampleTables: [],
}

export function LearningWorkspace({
  mode,
  problemKey,
}: {
  mode: "problems" | "solve"
  problemKey?: string
}) {
  const [packs, setPacks] = useState<InstalledPack[]>([])
  const [environments, setEnvironments] = useState<ManagedEnvironmentSummary[]>([])
  const [history, setHistory] = useState<SubmissionHistorySummary[]>([])
  const [historyToken, setHistoryToken] = useState(0)
  const [, setDraftVersion] = useState(0)
  const [result, setResult] = useState<JudgeRunResult | null>(null)
  const [resultMode, setResultMode] = useState<"run" | "submit" | null>(null)
  const [busy, setBusy] = useState(false)
  const [activeRequestId, setActiveRequestId] = useState<string | null>(null)
  const [activeMode, setActiveMode] = useState<"run" | "submit" | null>(null)
  const [elapsedBusyMs, setElapsedBusyMs] = useState(0)
  const [catalogLoading, setCatalogLoading] = useState(true)
  const [catalogError, setCatalogError] = useState<string | null>(null)
  const [catalogReloadToken, setCatalogReloadToken] = useState(0)
  const selectedProblemIdRef = useRef("")
  const judgeGenerationRef = useRef(0)
  const activeJudgeRequestRef = useRef<JudgeRequest | null>(null)
  const activeRequestIdRef = useRef<string | null>(null)

  function invalidateJudgeRequests() {
    judgeGenerationRef.current += 1
    activeJudgeRequestRef.current = null
    activeRequestIdRef.current = null
    setBusy(false)
    setActiveRequestId(null)
    setActiveMode(null)
    setResult(null)
    setResultMode(null)
  }

  function retryCatalogLoad() {
    invalidateJudgeRequests()
    setCatalogLoading(true)
    setCatalogError(null)
    setCatalogReloadToken((current) => current + 1)
  }

  useEffect(() => {
    let active = true

    void listManagedEnvironments()
      .then((managed) => active && setEnvironments(managed))
      .catch(() => active && setEnvironments([]))

    void listInstalledPacks()
      .then((installedPacks) => {
        if (active) {
          setPacks(installedPacks)
        }
      })
      .catch(() => {
        if (active) {
          setPacks([])
          setCatalogError(learningLoadErrorMessage("catalog"))
        }
      })
      .finally(() => {
        if (active) {
          setCatalogLoading(false)
        }
      })

    return () => {
      active = false
    }
  }, [catalogReloadToken])

  useEffect(() => {
    let active = true
    void listSubmissionHistory()
      .then((records) => active && setHistory(submissionsOnly(records)))
      .catch(() => active && setHistory([]))
    return () => {
      active = false
    }
  }, [historyToken])

  const learningProblems = useMemo(() => catalogProblems(packs), [packs])
  const selectedId = mode === "solve" ? (problemKey ?? "") : ""
  const selectedProblem =
    learningProblems.find((problem) => problem.id === selectedId) ??
    missingProblem
  const draft = sessionDrafts.get(selectedProblem.id, selectedProblem.starterSql ?? "")
  const judgeable = canJudgeProblem(selectedProblem)
  const completionItems = useMemo(
    () => sqlCompletionItems(selectedProblem.dialect, selectedProblem.schema),
    [selectedProblem.dialect, selectedProblem.schema]
  )

  useEffect(() => {
    if (selectedProblemIdRef.current !== selectedProblem.id) {
      selectedProblemIdRef.current = selectedProblem.id
      invalidateJudgeRequests()
    }
  }, [selectedProblem.id])

  useEffect(() => {
    if (!busy) {
      return
    }
    const startedAt = Date.now()
    const timer = window.setInterval(() => setElapsedBusyMs(Date.now() - startedAt), 100)
    return () => window.clearInterval(timer)
  }, [busy])

  function updateDraft(value: string | undefined) {
    sessionDrafts.set(selectedProblem.id, value ?? "")
    setDraftVersion((current) => current + 1)
  }

  async function run(mode: "run" | "submit") {
    if (!selectedProblem.id || !judgeable) {
      return
    }
    const problemIdentity = selectedProblem.id
    const request = beginJudgeRequest(
      judgeGenerationRef.current,
      problemIdentity
    )
    judgeGenerationRef.current = request.generation
    const requestId = crypto.randomUUID()
    activeJudgeRequestRef.current = request
    activeRequestIdRef.current = requestId
    setBusy(true)
    setElapsedBusyMs(0)
    setActiveRequestId(requestId)
    setActiveMode(mode)
    setResult(null)
    setResultMode(mode)
    try {
      const nextResult = await runJudge({
        problemId: selectedProblem.problemId,
        packId: selectedProblem.packId,
        packVersion: selectedProblem.packVersion,
        packDigest: selectedProblem.packDigest,
        sql: draft,
        mode,
        requestId,
      })
      if (
        isCurrentJudgeRequest(
          request,
          judgeGenerationRef.current,
          selectedProblemIdRef.current
        )
      ) {
        setResult(nextResult)
      }
      if (mode === "submit") {
        setHistoryToken((current) => current + 1)
      }
    } catch {
      if (
        isCurrentJudgeRequest(
          request,
          judgeGenerationRef.current,
          selectedProblemIdRef.current
        )
      ) {
        setResult({
          status: "error",
          durationMs: 0,
          message:
            "The native judge returned an unavailable or malformed response.",
        })
      }
    } finally {
      if (
        isCurrentJudgeRequest(
          request,
          judgeGenerationRef.current,
          selectedProblemIdRef.current
        )
      ) {
        activeJudgeRequestRef.current = null
        activeRequestIdRef.current = null
        setBusy(false)
        setActiveRequestId(null)
        setActiveMode(null)
      }
    }
  }

  async function cancelActiveJudge() {
    const request = activeJudgeRequestRef.current
    const requestId = activeRequestIdRef.current
    if (request && requestId) {
      await cancelJudge(requestId).catch(() => undefined)
    }
  }

  if (mode === "problems") {
    return (
      <ProblemsList
        packs={packs}
        problems={learningProblems}
        history={history}
        loading={catalogLoading}
        error={catalogError}
        onRetry={retryCatalogLoad}
      />
    )
  }

  const neighbours = adjacentProblems(learningProblems, selectedProblem.id)
  const packTitle =
    packs.find((pack) => pack.packId === selectedProblem.packId)?.title ??
    selectedProblem.packId

  return (
    <SolveView
      problem={selectedProblem}
      packTitle={packTitle}
      environment={environmentForProblem(environments, selectedProblem)}
      history={history}
      position={neighbours.position}
      total={learningProblems.length}
      previousHref={neighbours.previous ? solveHref(neighbours.previous.id) : undefined}
      nextHref={neighbours.next ? solveHref(neighbours.next.id) : undefined}
      loading={catalogLoading}
      error={catalogError}
      onRetry={retryCatalogLoad}
      draft={draft}
      onDraftChange={updateDraft}
      onResetDraft={() => updateDraft(selectedProblem.starterSql ?? "")}
      completionItems={completionItems}
      canJudge={Boolean(selectedProblem.id) && judgeable}
      busy={busy}
      activeMode={activeMode}
      canStop={busy && activeRequestId !== null && stopVisibleAfter(elapsedBusyMs)}
      result={result}
      resultMode={resultMode}
      onRun={(nextMode) => void run(nextMode)}
      onStop={() => void cancelActiveJudge()}
    />
  )
}

function solveHref(problemId: string) {
  return `#solve/${encodeURIComponent(problemId)}`
}
