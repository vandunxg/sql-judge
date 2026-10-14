import { ChevronLeft, ChevronRight } from "lucide-react"

import { Button } from "@/components/ui/button"

import type { paginateProblems } from "./practice-workspace-state"
import { pageWindow } from "./problems-filter"

type ProblemsPaginationProps = {
  paged: ReturnType<typeof paginateProblems>
  total: number
  onPage: (page: number) => void
}

export function ProblemsPagination({ paged, total, onPage }: ProblemsPaginationProps) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-3 border-t px-4 py-3">
      <span className="text-[13px] text-muted-foreground">
        Showing {paged.start + 1}–{paged.end} of {total}
      </span>
      <nav aria-label="Pagination" className="flex gap-1">
        <Button variant="ghost" size="icon" aria-label="Previous page" disabled={paged.page === 0} onClick={() => onPage(paged.page - 1)}>
          <ChevronLeft />
        </Button>
        {pageWindow(paged.page, paged.totalPages).map((page) => (
          <Button
            key={page}
            variant={page === paged.page ? "outline" : "ghost"}
            size="icon"
            aria-current={page === paged.page ? "page" : undefined}
            onClick={() => onPage(page)}
          >
            {page + 1}
          </Button>
        ))}
        <Button
          variant="ghost"
          size="icon"
          aria-label="Next page"
          disabled={paged.page >= paged.totalPages - 1}
          onClick={() => onPage(paged.page + 1)}
        >
          <ChevronRight />
        </Button>
      </nav>
    </div>
  )
}
