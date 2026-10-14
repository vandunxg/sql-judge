import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"

import type { ProblemDifficulty } from "./catalog"

const difficultyClass: Record<ProblemDifficulty, string> = {
  easy: "bg-ok-bg text-ok",
  medium: "bg-warn-bg text-warn",
  hard: "bg-err-bg text-err",
}

export function DifficultyBadge({ difficulty }: { difficulty: ProblemDifficulty }) {
  return (
    <Badge className={cn("border-transparent", difficultyClass[difficulty])}>
      {difficulty[0].toUpperCase() + difficulty.slice(1)}
    </Badge>
  )
}
