import * as React from "react"
import {
  BarChart3,
  BookOpen,
  Cable,
  Database,
  History,
  Home,
  Package,
  Search,
  SlidersHorizontal,
  Table2,
} from "lucide-react"

import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar"
import type { AppRoute } from "@/app-route-state"
import { getManagedRuntimeStatus, type ManagedRuntimeStatus } from "@/lib/judge-api"
import { cn } from "@/lib/utils"

type NavigationItem = {
  route: AppRoute
  label: string
  icon: typeof Home
}

const navigation: NavigationItem[] = [
  { route: "home", label: "Home", icon: Home },
  { route: "problems", label: "Problems", icon: BookOpen },
  { route: "history", label: "History", icon: History },
  { route: "progress", label: "Progress", icon: BarChart3 },
]

const libraryNavigation: NavigationItem[] = [
  { route: "packs", label: "Packs", icon: Package },
]

const dataNavigation: NavigationItem[] = [
  { route: "environments", label: "Environments", icon: Database },
  { route: "connections", label: "Connections", icon: Cable },
  { route: "explorer", label: "Explorer", icon: Table2 },
]

function isActive(item: AppRoute, activeRoute: AppRoute) {
  return item === activeRoute || (item === "problems" && activeRoute === "practice")
}

export function AppSidebar({
  activeRoute,
  ...props
}: React.ComponentProps<typeof Sidebar> & { activeRoute: AppRoute }) {
  return (
    <Sidebar collapsible="offcanvas" {...props}>
      <SidebarHeader>
        <SidebarMenu className="mt-1">
          <SidebarMenuItem>
            <SidebarMenuButton
              className="h-auto! data-[slot=sidebar-menu-button]:p-1.5!"
              render={<a href="#home" />}
              isActive={activeRoute === "home"}
            >
              <span className="flex size-7 items-center justify-center rounded-lg bg-sidebar-primary font-mono text-xs font-medium text-sidebar-primary-foreground">SJ</span>
              <span className="flex flex-col leading-tight"><span className="text-base font-semibold">SQL Judge</span><span className="text-xs font-normal text-sidebar-foreground/60">Local workspace</span></span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup className="pb-1">
          <SidebarGroupContent>
            <button
              type="button"
              className="flex h-[34px] w-full items-center justify-between rounded-lg border border-sidebar-border bg-sidebar px-3 text-sm text-sidebar-foreground/60 shadow-none hover:bg-sidebar-accent focus-visible:ring-2 focus-visible:ring-ring"
            >
              <span className="flex items-center gap-2"><Search className="size-4" /><span>Search</span></span>
              <kbd className="h-5 rounded-[5px] border border-sidebar-border bg-sidebar px-1.5 font-mono text-[11px] leading-5">Ctrl K</kbd>
            </button>
          </SidebarGroupContent>
        </SidebarGroup>
        <SidebarGroup>
          <SidebarGroupLabel>Learn</SidebarGroupLabel>
          <SidebarGroupContent>
            <NavigationItems items={navigation} activeRoute={activeRoute} />
          </SidebarGroupContent>
        </SidebarGroup>
        <SidebarGroup>
          <SidebarGroupLabel>Library</SidebarGroupLabel>
          <SidebarGroupContent>
            <NavigationItems items={libraryNavigation} activeRoute={activeRoute} />
          </SidebarGroupContent>
        </SidebarGroup>
        <SidebarGroup>
          <SidebarGroupLabel>Data</SidebarGroupLabel>
          <SidebarGroupContent>
            <NavigationItems items={dataNavigation} activeRoute={activeRoute} />
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter className="gap-1 pb-3">
        <RuntimeStatus />
        <NavigationItems
          items={[{ route: "settings", label: "Settings", icon: SlidersHorizontal }]}
          activeRoute={activeRoute}
        />
      </SidebarFooter>
    </Sidebar>
  )
}

function NavigationItems({
  items,
  activeRoute,
}: {
  items: NavigationItem[]
  activeRoute: AppRoute
}) {
  return (
    <SidebarMenu>
      {items.map((item) => {
        const Icon = item.icon
        return (
          <SidebarMenuItem key={item.route}>
            <SidebarMenuButton
              render={<a href={`#${item.route}`} />}
              isActive={isActive(item.route, activeRoute)}
              tooltip={item.label}
            >
              <Icon />
              <span>{item.label}</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        )
      })}
    </SidebarMenu>
  )
}

function RuntimeStatus() {
  const [status, setStatus] = React.useState<ManagedRuntimeStatus | null>(null)

  React.useEffect(() => {
    let active = true
    void getManagedRuntimeStatus()
      .then((next) => active && setStatus(next))
      .catch(() => active && setStatus(null))
    return () => {
      active = false
    }
  }, [])

  return (
    <a
      href="#environments"
      className="flex gap-2.5 rounded-lg px-2 py-1.5 text-xs hover:bg-sidebar-accent"
    >
      <span
        className={cn(
          "mt-1.5 size-2 shrink-0 rounded-full",
          status?.available ? "bg-ok" : "bg-warn"
        )}
      />
      <span className="min-w-0">
        <span className="block font-medium">
          {status?.available ? "Runtime ready" : "Runtime stopped"}
        </span>
        <span className="block truncate text-sidebar-foreground/60">
          {status?.available ? "PostgreSQL · local socket" : "Start it from Environments"}
        </span>
      </span>
    </a>
  )
}
