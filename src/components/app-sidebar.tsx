import * as React from "react"
import {
  BookOpen,
  ChartColumn,
  Database,
  History,
  House,
  Package,
  Plug,
  Search,
  Settings2,
  Table,
  type LucideIcon,
} from "lucide-react"

import { Sidebar } from "@/components/ui/sidebar"
import { getManagedRuntimeStatus, type ManagedRuntimeStatus } from "@/lib/judge-api"
import { cn } from "@/lib/utils"
import { runtimeIndicator } from "./runtime-indicator"

export type AppRoute =
  | "home"
  | "problems"
  | "solve"
  | "environments"
  | "history"
  | "progress"
  | "packs"
  | "connections"
  | "explorer"
  | "settings"

type NavigationItem = {
  route: AppRoute
  label: string
  icon: LucideIcon
}

const navigationGroups: Array<{ label: string; items: NavigationItem[] }> = [
  {
    label: "Learn",
    items: [
      { route: "home", label: "Home", icon: House },
      { route: "problems", label: "Problems", icon: BookOpen },
      { route: "history", label: "History", icon: History },
      { route: "progress", label: "Progress", icon: ChartColumn },
    ],
  },
  {
    label: "Library",
    items: [{ route: "packs", label: "Packs", icon: Package }],
  },
  {
    label: "Data",
    items: [
      { route: "environments", label: "Environments", icon: Database },
      { route: "connections", label: "Connections", icon: Plug },
      { route: "explorer", label: "Explorer", icon: Table },
    ],
  },
]

const focusRing =
  "outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring"
const runtimeDots = { ok: "bg-[#16a34a]", err: "bg-err", muted: "bg-muted-foreground" }

export function AppSidebar({
  activeRoute,
  ...props
}: React.ComponentProps<typeof Sidebar> & { activeRoute: AppRoute }) {
  const [runtime, setRuntime] = React.useState<ManagedRuntimeStatus | null | undefined>()
  const indicator = runtimeIndicator(runtime)

  React.useEffect(() => {
    let active = true
    void getManagedRuntimeStatus()
      .then((status) => active && setRuntime(status))
      .catch(() => active && setRuntime(null))
    return () => {
      active = false
    }
  }, [activeRoute])

  return (
    <Sidebar collapsible="offcanvas" {...props}>
      <nav
        aria-label="Main navigation"
        className="flex h-full flex-col gap-5 px-3 py-3.5 text-sm text-foreground"
      >
        <a href="#home" className={cn("flex items-center gap-2.5 rounded-lg px-1.5 py-0.5", focusRing)}>
          <span className="flex size-[30px] items-center justify-center rounded-lg bg-primary font-mono text-xs font-medium text-primary-foreground">
            SJ
          </span>
          <span className="flex flex-col leading-[1.25]">
            <span className="font-semibold">SQL Judge</span>
            <span className="text-xs text-muted-foreground">Local workspace</span>
          </span>
        </a>
        <button
          type="button"
          disabled
          title="Command search is not available yet"
          className="flex h-[34px] w-full items-center justify-between rounded-lg border border-input bg-card px-3 text-muted-foreground"
        >
          <span className="flex items-center gap-2">
            <Search className="size-4" aria-hidden="true" />
            Search
          </span>
          <kbd className="inline-flex h-5 items-center rounded-[5px] border bg-card px-[5px] font-mono text-[11px]">
            Ctrl K
          </kbd>
        </button>
        <div className="flex flex-col gap-[18px]">
          {navigationGroups.map((group) => (
            <div key={group.label} className="flex flex-col gap-0.5">
              <div className="px-2.5 pb-1 text-xs font-medium text-muted-foreground">{group.label}</div>
              {group.items.map((item) => (
                <NavLink key={item.route} item={item} active={activeRoute === item.route || (item.route === "problems" && activeRoute === "solve")} />
              ))}
            </div>
          ))}
        </div>
        <div className="flex-1" />
        <div className="flex flex-col gap-0.5">
          <a
            href="#environments"
            className={cn("flex items-start gap-2.5 rounded-lg px-2.5 py-2 hover:bg-hover", focusRing)}
          >
            <span className={cn("mt-[7px] size-2 shrink-0 rounded-full", runtimeDots[indicator.tone])} aria-hidden="true" />
            <span className="flex flex-col leading-[1.35]">
              <span className="text-[13px] font-medium">{indicator.label}</span>
              <span className="text-xs text-muted-foreground">{indicator.detail}</span>
            </span>
          </a>
          <NavLink
            item={{ route: "settings", label: "Settings", icon: Settings2 }}
            active={activeRoute === "settings"}
          />
        </div>
      </nav>
    </Sidebar>
  )
}

function NavLink({ item, active }: { item: NavigationItem; active: boolean }) {
  const Icon = item.icon
  return (
    <a
      href={`#${item.route}`}
      aria-current={active ? "page" : undefined}
      className={cn(
        "flex h-[34px] items-center gap-2.5 rounded-lg px-2.5 hover:bg-hover",
        focusRing,
        active && "bg-hover font-medium"
      )}
    >
      <Icon className={cn("size-4 shrink-0", active ? "text-foreground" : "text-muted-foreground")} aria-hidden="true" />
      {item.label}
    </a>
  )
}
