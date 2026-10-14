import { useEffect, useRef, useState } from "react"
import { cn } from "cn"

import { SettingsBackup } from "./settings-backup"
import { SettingsAbout, SettingsJudgeLimits, SettingsPrivacy } from "./settings-info"
import { SettingsAppearance, SettingsEditor } from "./settings-preferences"
import { settingsFocusRing } from "./settings-rows"
import { activeSectionId, settingsSections, type SettingsSectionId } from "./settings-state"

const ACTIVE_THRESHOLD_PX = 120

function scrolledToEnd(start: Element | null) {
  const scrollable: Element[] = []
  for (let element = start; element; element = element.parentElement) {
    if (element.scrollHeight > element.clientHeight + 1 && getComputedStyle(element).overflowY !== "visible") {
      scrollable.push(element)
    }
  }
  const root = document.scrollingElement
  if (root && root.scrollHeight > root.clientHeight + 1) {
    scrollable.push(root)
  }
  return (
    scrollable.length > 0 &&
    scrollable.every((element) => element.scrollTop + element.clientHeight >= element.scrollHeight - 2)
  )
}

export function SettingsScreen() {
  const mainRef = useRef<HTMLElement>(null)
  const pinnedRef = useRef<SettingsSectionId | null>(null)
  const [active, setActive] = useState<SettingsSectionId>(settingsSections[0].id)

  useEffect(() => {
    function sync() {
      if (pinnedRef.current) {
        return
      }
      const sections = settingsSections.flatMap(({ id }) => {
        const element = document.getElementById(id)
        return element ? [{ id, top: element.getBoundingClientRect().top }] : []
      })
      const atEnd = scrolledToEnd(mainRef.current)
      const next = activeSectionId(sections, { offset: 0, threshold: ACTIVE_THRESHOLD_PX, atEnd })
      if (next) {
        setActive(next)
      }
    }
    function release() {
      pinnedRef.current = null
    }
    const userScrollEvents = ["wheel", "touchstart", "keydown"] as const
    sync()
    window.addEventListener("scroll", sync, { capture: true, passive: true })
    window.addEventListener("resize", sync)
    userScrollEvents.forEach((name) => window.addEventListener(name, release, { passive: true }))
    return () => {
      window.removeEventListener("scroll", sync, { capture: true })
      window.removeEventListener("resize", sync)
      userScrollEvents.forEach((name) => window.removeEventListener(name, release))
    }
  }, [])

  function goTo(id: SettingsSectionId) {
    pinnedRef.current = id
    setActive(id)
    const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches
    document.getElementById(id)?.scrollIntoView({ behavior: reduceMotion ? "auto" : "smooth", block: "start" })
  }

  return (
    <main ref={mainRef} className="flex-1 px-4 pt-8 pb-14 sm:px-10">
      <div className="mx-auto flex max-w-[1040px] flex-col gap-7">
        <header className="flex flex-col gap-1.5">
          <h1 className="text-[26px] leading-[1.2] font-semibold tracking-[-0.02em]">Settings</h1>
          <p className="text-[15px] text-muted-foreground">Changes save as you go.</p>
        </header>

        <div className="flex flex-wrap items-start gap-8">
          <nav aria-label="Settings sections" className="flex flex-[1_1_180px] flex-col gap-0.5 xl:sticky xl:top-6">
            {settingsSections.map(({ id, label }) => (
              <button
                key={id}
                type="button"
                aria-current={active === id ? "true" : undefined}
                onClick={() => goTo(id)}
                className={cn(
                  "flex h-[34px] cursor-pointer items-center rounded-lg px-3 text-left text-sm text-muted-foreground hover:bg-accent hover:text-foreground",
                  settingsFocusRing,
                  active === id && "bg-accent font-medium text-foreground"
                )}
              >
                {label}
              </button>
            ))}
          </nav>

          <div className="flex min-w-0 flex-[999_1_520px] flex-col gap-8">
            <SettingsAppearance />
            <SettingsEditor />
            <SettingsJudgeLimits />
            <SettingsBackup />
            <SettingsPrivacy />
            <SettingsAbout />
          </div>
        </div>
      </div>
    </main>
  )
}
