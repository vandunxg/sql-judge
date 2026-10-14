import { useEffect, useState, type CSSProperties } from "react"

import { AppSidebar } from "@/components/app-sidebar"
import { SiteHeader } from "@/components/site-header"
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar"
import { TooltipProvider } from "@/components/ui/tooltip"
import { parseHash, routeForLaunch, type AppRoute } from "@/app-route-state"
import { ConnectionsScreen } from "@/features/learning/connections-screen"
import { EnvironmentsScreen } from "@/features/learning/environments-screen"
import { ExplorerScreen } from "@/features/learning/explorer-screen"
import { HistoryScreen } from "@/features/learning/history-screen"
import { HomeScreen } from "@/features/learning/home-screen"
import { LearningWorkspace } from "@/features/learning/learning-workspace"
import { PacksScreen } from "@/features/learning/packs-screen"
import { ProblemsScreen } from "@/features/learning/problems-screen"
import { ProgressScreen } from "@/features/learning/progress-screen"
import { SettingsScreen } from "@/features/learning/settings-screen"

const routeTitles: Record<AppRoute, string> = {
  home: "Home",
  problems: "Problems",
  practice: "Solve",
  environments: "Environments",
  history: "History",
  progress: "Progress",
  packs: "Packs",
  connections: "Connections",
  explorer: "Explorer",
  settings: "Settings",
}

export default function App() {
  const [route, setRoute] = useState<AppRoute>(() => routeForLaunch())

  useEffect(() => {
    const handleHashChange = () => setRoute(parseHash(window.location.hash).route)
    window.addEventListener("hashchange", handleHashChange)
    if (window.location.hash !== "#home") {
      window.history.replaceState(null, "", "#home")
    }
    return () => window.removeEventListener("hashchange", handleHashChange)
  }, [])

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
        <AppSidebar activeRoute={route} variant="inset" />
        <SidebarInset className="min-w-0">
          <SiteHeader title={routeTitles[route]} />
          {route === "home" ? <HomeScreen /> : null}
          {route === "problems" ? <ProblemsScreen /> : null}
          {route === "practice" ? <LearningWorkspace /> : null}
          {route === "environments" ? <EnvironmentsScreen /> : null}
          {route === "history" ? <HistoryScreen /> : null}
          {route === "progress" ? <ProgressScreen /> : null}
          {route === "packs" ? <PacksScreen /> : null}
          {route === "connections" ? <ConnectionsScreen /> : null}
          {route === "explorer" ? <ExplorerScreen /> : null}
          {route === "settings" ? <SettingsScreen /> : null}
        </SidebarInset>
      </SidebarProvider>
    </TooltipProvider>
  )
}
