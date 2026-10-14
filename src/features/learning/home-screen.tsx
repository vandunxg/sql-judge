import { useEffect, useState } from "react"
import {
  ArrowRight,
  CheckCircle2,
  CircleDashed,
  CircleDot,
  LoaderCircle,
} from "lucide-react"

import { problemHref } from "@/app-route-state"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import {
  listInstalledPacks,
  listSubmissionHistory,
  type InstalledPack,
  type SubmissionHistorySummary,
} from "@/lib/judge-api"

import { catalogProblems, type LearningProblem } from "./catalog"
import { DifficultyBadge } from "./difficulty-badge"
import { homeProblems } from "./home-state"
import { problemStatuses, resumeProblemId } from "./problem-status"
import { learningLoadErrorMessage } from "./load-state"

const homeCardClass =
  "gap-0 rounded-xl border border-border bg-card py-0 shadow-[0_1px_2px_rgba(16,16,24,0.05)] dark:shadow-[0_1px_2px_rgba(0,0,0,0.5)]"

export function HomeScreen() {
  const [packs, setPacks] = useState<InstalledPack[]>([])
  const [history, setHistory] = useState<SubmissionHistorySummary[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let active = true
    void listSubmissionHistory()
      .then((records) => active && setHistory(records))
      .catch(() => undefined)
    void listInstalledPacks()
      .then((nextPacks) => active && setPacks(nextPacks))
      .catch(() => active && setError(learningLoadErrorMessage("catalog")))
      .finally(() => active && setLoading(false))
    return () => {
      active = false
    }
  }, [])

  const problems = catalogProblems(packs)
  const resumeId = resumeProblemId(problemStatuses(history)) ?? ""
  const resumePackId = problems.find((problem) => problem.id === resumeId)?.packId
  const activePack = packs.find((pack) => pack.packId === resumePackId) ?? packs[0]
  const { resume, next } = homeProblems(
    problems,
    activePack?.packId ?? "",
    resumeId
  )

  return (
    <main className="min-h-0 flex-1 overflow-auto px-4 py-8 sm:px-8 lg:px-10 lg:pt-10 lg:pb-14">
      <div className="mx-auto flex max-w-[1080px] flex-col gap-8">
        <header className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <h1 className="text-2xl font-semibold tracking-tight">Welcome back</h1>
            <p className="mt-1 text-[15px] text-muted-foreground">
              {problems.length > 0
                ? "Choose a problem and keep your SQL practice moving."
                : "Import a validated problem pack to begin practicing."}
            </p>
          </div>
          <Button className="h-9 gap-2 px-3.5" variant="outline" nativeButton={false} render={<a href="#problems" />}>
            Browse problems <ArrowRight />
          </Button>
        </header>

        {loading ? (
          <Card className={`${homeCardClass} p-6 text-sm text-muted-foreground`}>
            <LoaderCircle className="mr-2 inline size-4 animate-spin" />
            Loading local learning content...
          </Card>
        ) : error ? (
          <Card className={`${homeCardClass} border-destructive/30 p-6 text-sm text-destructive`} role="alert">
            {error}
          </Card>
        ) : (
          <div className="grid gap-5 lg:grid-cols-[minmax(0,2fr)_minmax(18rem,1fr)]">
            <div className="space-y-5">
              <ResumeCard problem={resume} pack={activePack} />
              <UpNext problems={next} pack={activePack} />
            </div>
            <div className="space-y-5">
              <SetupCard hasPacks={packs.length > 0} />
              <ShortcutsCard />
            </div>
          </div>
        )}
      </div>
    </main>
  )
}

function ResumeCard({ problem, pack }: { problem?: LearningProblem; pack?: InstalledPack }) {
  return (
    <Card className={homeCardClass}>
      <CardContent className="flex flex-col gap-5 p-6">
        <div className="flex flex-wrap items-center justify-between gap-2 text-[13px] text-muted-foreground">
          <span className="font-medium">{problem ? "Start your next problem" : "Your workspace is ready"}</span>
          {pack ? <span>{pack.title}</span> : null}
        </div>
        {problem ? (
          <>
            <div>
              <h2 className="text-xl font-semibold tracking-tight">{problem.title}</h2>
              <div className="mt-3 flex flex-wrap items-center gap-1.5">
                <DifficultyBadge difficulty={problem.difficulty} />
                {problem.topics.slice(0, 2).map((topic) => <Badge key={topic} variant="outline">{topic}</Badge>)}
              </div>
            </div>
            <pre className="overflow-hidden rounded-lg bg-muted p-4 font-mono text-[13px] leading-6 text-muted-foreground whitespace-pre-wrap">
              {problem.starterSql || "-- Write a read-only SQL statement\nSELECT ..."}
            </pre>
            <div className="flex flex-wrap gap-2">
              <Button className="h-9 gap-2 px-3.5" nativeButton={false} render={<a href={problemHref(problem.id)} />}>Start solving <ArrowRight /></Button>
              <Button className="h-9 gap-2 px-3.5" variant="ghost" nativeButton={false} render={<a href="#problems" />}>Pick another problem</Button>
            </div>
          </>
        ) : (
          <div className="flex flex-col items-start gap-4">
            <p className="max-w-xl text-sm leading-6 text-muted-foreground">Import a validated pack, then use Practice to run and submit a SQL solution against isolated fixtures.</p>
            <Button className="h-9 gap-2 px-3.5" nativeButton={false} render={<a href="#packs" />}>Open packs <ArrowRight /></Button>
          </div>
        )}
      </CardContent>
    </Card>
  )
}

function UpNext({ problems, pack }: { problems: LearningProblem[]; pack?: InstalledPack }) {
  return (
    <Card className={`${homeCardClass} overflow-hidden`}>
      <CardHeader className="!flex flex-row items-center justify-between gap-3 border-b px-5 py-4">
        <CardTitle className="min-w-0 flex-1 truncate text-[15px]">Up next{pack ? ` in ${pack.title}` : ""}</CardTitle>
        <Button className="h-8 shrink-0 gap-1.5 px-2.5" variant="outline" render={<a href="#practice" />}>View all</Button>
      </CardHeader>
      <CardContent className="p-0">
        {problems.length > 0 ? problems.map((problem) => (
          <a key={problem.id} href={problemHref(problem.id)} className="flex items-center gap-3 border-b px-5 py-3 last:border-b-0 hover:bg-muted/60">
            <CircleDashed className="size-4 shrink-0 text-muted-foreground" />
            <span className="min-w-0 flex-1"><span className="block truncate font-medium">{problem.title}</span><span className="mt-0.5 block text-[13px] text-muted-foreground">{problem.topics.join(" · ") || problem.dialect}</span></span>
            <DifficultyBadge difficulty={problem.difficulty} />
            <ArrowRight className="size-4 text-muted-foreground" />
          </a>
        )) : <p className="p-5 text-sm text-muted-foreground">More problems will appear after a pack is imported.</p>}
      </CardContent>
    </Card>
  )
}

function SetupCard({ hasPacks }: { hasPacks: boolean }) {
  const complete = hasPacks ? 2 : 1
  return <Card className={homeCardClass}><CardContent className="space-y-5 p-5"><div><div className="flex items-center justify-between"><h2 className="text-[15px] font-semibold">Get set up</h2><span className="text-[13px] text-muted-foreground">{complete} of 3 done</span></div><div className="mt-3 h-1.5 overflow-hidden rounded-full bg-muted"><div className="h-full rounded-full bg-ok" style={{ width: `${(complete / 3) * 100}%` }} /></div></div><SetupStep done label="Open the local workspace" detail="Your practice workspace runs on this device." /><SetupStep done={hasPacks} label="Import a problem pack" detail={hasPacks ? "Validated learning content is available." : "Add a validated pack to unlock practice."} /><SetupStep label="Submit your first solution" detail="Run a sample, then submit for judging." action="#problems" /></CardContent></Card>
}

function SetupStep({ done = false, label, detail, action }: { done?: boolean; label: string; detail: string; action?: string }) {
  return <div className="flex gap-3"><span className="mt-0.5">{done ? <CheckCircle2 className="size-5 text-ok" /> : <CircleDot className="size-5" />}</span><div><p className="text-sm font-medium">{label}</p><p className="mt-0.5 text-[13px] leading-5 text-muted-foreground">{detail}</p>{action ? <Button className="mt-3 h-8 gap-1.5 px-2.5" variant="outline" nativeButton={false} render={<a href={action} />}>Open a problem</Button> : null}</div></div>
}

function ShortcutsCard() {
  return <Card className={homeCardClass}><CardContent className="p-5"><h2 className="text-[15px] font-semibold">Handy shortcuts</h2><dl className="mt-4 space-y-3 text-[13px]"><Shortcut label="Search anything" keys={["Ctrl", "K"]} /><Shortcut label="Toggle sidebar" keys={["Ctrl", "B"]} /></dl></CardContent></Card>
}

function Shortcut({ label, keys }: { label: string; keys: string[] }) {
  return <div className="flex items-center justify-between gap-3"><dt className="text-muted-foreground">{label}</dt><dd className="flex gap-1">{keys.map((key) => <kbd key={key} className="rounded border bg-muted px-1.5 py-0.5 font-mono text-[11px] text-muted-foreground">{key}</kbd>)}</dd></div>
}

