import { useEffect, useState } from "react"
import { LoaderCircle, RefreshCw } from "lucide-react"

import { parseHash } from "@/app-route-state"
import { Button } from "@/components/ui/button"
import { useAppSettings } from "@/lib/app-settings"
import { listInstalledPacks, type InstalledPack } from "@/lib/judge-api"

import {
  canJudgeProblem,
  catalogProblems,
  hasInstalledProblems,
} from "./catalog"
import { loadDrafts, saveDrafts } from "./draft-store"
import { EditorPane } from "./editor-pane"
import { learningLoadErrorMessage } from "./load-state"
import { adjacentProblemId, problemsForPack } from "./practice-workspace-state"
import { ProblemPane } from "./problem-pane"
import { ResultPane } from "./result-pane"
import { SolveToolbar } from "./solve-toolbar"
import { useJudgeRun, type JudgeMode } from "./use-judge-run"

const MAX_SQL_LENGTH = 65_536

export function LearningWorkspace() {
  const [packs, setPacks] = useState<InstalledPack[]>([])
  const [selectedId, setSelectedId] = useState("")
  const { keepDrafts } = useAppSettings()
  const [drafts, setDrafts] = useState<Record<string, string>>(() => (keepDrafts ? loadDrafts() : {}))
  const [catalogLoading, setCatalogLoading] = useState(true)
  const [catalogError, setCatalogError] = useState<string | null>(null)
  const [catalogReloadToken, setCatalogReloadToken] = useState(0)
  const judge = useJudgeRun()
  const { trackProblem, trackedProblemId } = judge

  useEffect(() => {
    let active = true
    void listInstalledPacks()
      .then((installedPacks) => {
        if (!active) {
          return
        }
        const problems = catalogProblems(installedPacks)
        const requested = parseHash(window.location.hash).params.get("problem")
        const nextId = (
          problems.find((problem) => problem.id === trackedProblemId()) ??
          problems.find((problem) => problem.id === requested) ??
          problems[0]
        )?.id ?? ""
        setPacks(installedPacks)
        trackProblem(nextId)
        setSelectedId(nextId)
      })
      .catch(() => {
        if (!active) {
          return
        }
        setPacks([])
        trackProblem("")
        setSelectedId("")
        setCatalogError(learningLoadErrorMessage("catalog"))
      })
      .finally(() => active && setCatalogLoading(false))
    return () => {
      active = false
    }
  }, [catalogReloadToken, trackProblem, trackedProblemId])

  useEffect(() => saveDrafts(keepDrafts ? drafts : null), [drafts, keepDrafts])

  const learningProblems = catalogProblems(packs)
  const problem = learningProblems.find((candidate) => candidate.id === selectedId) ?? null
  const packId = problem?.packId ?? ""
  const packProblems = problemsForPack(learningProblems, packId)
  const pack = packs.find(
    (candidate) => candidate.packId === packId && candidate.version === problem?.packVersion
  )
  const draft = problem ? (drafts[problem.id] ?? problem.starterSql ?? "") : ""
  const canJudge = problem !== null && canJudgeProblem(problem) && !catalogLoading && !catalogError

  function selectProblem(problemId: string) {
    trackProblem(problemId)
    setSelectedId(problemId)
  }

  function setDraft(value: string) {
    if (problem) {
      setDrafts((current) => ({ ...current, [problem.id]: value.slice(0, MAX_SQL_LENGTH) }))
    }
  }

  function runJudge(mode: JudgeMode) {
    if (problem && canJudge && !judge.busy) {
      void judge.run(problem, draft, mode)
    }
  }

  function retryCatalogLoad() {
    judge.invalidate()
    setCatalogLoading(true)
    setCatalogError(null)
    setCatalogReloadToken((current) => current + 1)
  }

  const notice = catalogLoading ? (
    <p className="flex items-center gap-2 text-[13px] text-muted-foreground">
      <LoaderCircle className="size-4 animate-spin" /> Loading installed problem packs...
    </p>
  ) : catalogError ? (
    <div className="flex flex-col items-start gap-3 text-[13px]" role="alert">
      <p className="text-destructive">{catalogError}</p>
      <Button variant="outline" size="sm" onClick={retryCatalogLoad}>
        <RefreshCw /> Retry
      </Button>
    </div>
  ) : (
    <div className="flex flex-col items-start gap-3 text-[13px] text-muted-foreground">
      <p>No complete problem metadata is available. Import a validated pack with problem manifests to begin practicing.</p>
      <Button variant="outline" size="sm" nativeButton={false} render={<a href="#packs" />}>Open packs</Button>
    </div>
  )

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <SolveToolbar
        listProps={{
          packs: packs.filter(hasInstalledProblems),
          selectedPackId: packId,
          problems: packProblems,
          selectedId,
          disabled: judge.busy || learningProblems.length === 0,
          onSelectPack: (nextPackId) =>
            selectProblem(problemsForPack(learningProblems, nextPackId)[0]?.id ?? ""),
          onSelectProblem: selectProblem,
        }}
        previousId={adjacentProblemId(packProblems, selectedId, -1)}
        nextId={adjacentProblemId(packProblems, selectedId, 1)}
        activeMode={judge.activeMode}
        canJudge={canJudge}
        canStop={judge.canStop}
        onRun={runJudge}
        onStop={() => void judge.cancel()}
      />
      <div className="flex min-h-0 flex-1 flex-wrap gap-2 px-2.5 pb-2.5">
        <div className="flex min-h-[600px] min-w-0 flex-[1_1_440px]">
          <ProblemPane
            problem={problem}
            packLabel={pack ? `${pack.title} ${pack.version}` : ""}
            notice={notice}
          />
        </div>
        <div className="flex min-w-0 flex-[999_1_560px] flex-col gap-2">
          <EditorPane
            problem={problem}
            value={draft}
            readOnly={judge.busy || !problem}
            onChange={setDraft}
            onReset={() => problem && setDraft(problem.starterSql ?? "")}
            onRun={runJudge}
          />
          <ResultPane
            result={judge.result}
            resultMode={judge.resultMode}
            activeMode={judge.activeMode}
          />
        </div>
      </div>
    </div>
  )
}
