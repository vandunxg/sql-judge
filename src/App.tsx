import { useEffect, useState, type CSSProperties } from "react"

import { AppSidebar, type AppRoute } from "@/components/app-sidebar"
import { SiteHeader } from "@/components/site-header"
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar"
import { TooltipProvider } from "@/components/ui/tooltip"
import { routeForHash, routeForLaunch } from "@/app-route-state"
import { ConnectionsScreen } from "@/features/learning/connections-screen"
import { EnvironmentsScreen } from "@/features/learning/environments-screen"
import { ExplorerScreen } from "@/features/learning/explorer-screen"
import { HistoryScreen } from "@/features/learning/history-screen"
import { HomeScreen } from "@/features/learning/home-screen"
import { LearningWorkspace } from "@/features/learning/learning-workspace"
import { PacksScreen } from "@/features/learning/packs-screen"
import { ProgressScreen } from "@/features/learning/progress-screen"
import { SettingsScreen } from "@/features/learning/settings-screen"

const routeTitles: Record<AppRoute, string> = {
  home: "Home",
  problems: "Problems",
  solve: "Solve",
  environments: "Environments",
  history: "History",
  progress: "Progress",
  packs: "Packs",
  connections: "Connections",
  explorer: "Explorer",
  settings: "Settings",
}

function readRoute(): AppRoute {
  return routeForHash(window.location.hash)
}

function solveProblemKey() {
  const encodedKey = window.location.hash.slice("#solve/".length)
  try {
    return encodedKey ? decodeURIComponent(encodedKey) : undefined
  } catch {
    return undefined
  }
}

export default function App() {
  const [route, setRoute] = useState<AppRoute>(() => routeForLaunch(window.location.hash))

  useEffect(() => {
    const handleHashChange = () => setRoute(readRoute())
    window.addEventListener("hashchange", handleHashChange)
    if (!window.location.hash) {
      window.history.replaceState(null, "", "#home")
    }
    return () => window.removeEventListener("hashchange", handleHashChange)
  }, [])

  const screen = (
    <>
      {route === "home" ? <HomeScreen /> : null}
      {route === "problems" || route === "solve" ? (
        <LearningWorkspace
          mode={route === "solve" ? "solve" : "problems"}
          problemKey={route === "solve" ? solveProblemKey() : undefined}
        />
      ) : null}
      {route === "environments" ? <EnvironmentsScreen /> : null}
      {route === "history" ? <HistoryScreen /> : null}
      {route === "progress" ? <ProgressScreen /> : null}
      {route === "packs" ? <PacksScreen /> : null}
      {route === "connections" ? <ConnectionsScreen /> : null}
      {route === "explorer" ? <ExplorerScreen /> : null}
      {route === "settings" ? <SettingsScreen /> : null}
    </>
  )

  if (route === "solve") {
    return <TooltipProvider>{screen}</TooltipProvider>
  }

  return (
    <TooltipProvider>
      <SidebarProvider
        style={
          {
            "--sidebar-width": "15.5rem",
            "--header-height": "3.5rem",
          } as CSSProperties
        }
      >
        <AppSidebar activeRoute={route} />
        <SidebarInset className="min-w-0">
          <SiteHeader title={routeTitles[route]} />
          {screen}
        </SidebarInset>
      </SidebarProvider>
    </TooltipProvider>
  )
}
