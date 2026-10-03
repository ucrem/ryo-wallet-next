import type { ReactNode } from "react"
import type { WalletSection } from "@/api/operations"
import ryoMark from "@/assets/ryo-mark.svg"
import { navigation, type NavigationLayout, type Screen } from "./navigation"

export function LayoutPreviewSwitch({ layout, onChange }: {
  layout: NavigationLayout
  onChange: (layout: NavigationLayout) => void
}) {
  return (
    <div className="flex items-center gap-2" aria-label="Navigation layout comparison">
      <span className="text-[11px] text-slate-500">Layout preview</span>
      <div className="flex rounded-lg border border-slate-700 bg-[var(--app-bg)] p-1">
        {(["top", "sidebar"] as const).map((option) => (
          <button key={option} type="button" aria-pressed={layout === option}
            onClick={() => onChange(option)}
            className={"rounded-md px-3 py-1 text-xs transition-colors focus-visible:outline-2 focus-visible:outline-sky-400 " +
              (layout === option ? "bg-slate-700 text-white" : "text-slate-400 hover:text-slate-100")}>
            {option === "top" ? "Top menu" : "Sidebar"}
          </button>
        ))}
      </div>
    </div>
  )
}

export function TopNavigation({ screen, canVisit, onNavigate, walletOpen, walletSection, onWalletSection, versionLabel, updateVersion, comparison, networkLabel = "Mainnet" }: {
  screen: Screen
  canVisit: (screen: Screen) => boolean
  onNavigate: (screen: Screen) => void
  walletOpen: boolean
  walletSection: WalletSection
  onWalletSection: (section: WalletSection) => void
  versionLabel: string | null
  updateVersion: string | null
  comparison?: ReactNode
  networkLabel?: string
}) {
  const inSetup = screen === "storage" || screen === "node" || screen === "summary"
  const sections: { label: string; screen: Screen; active: boolean }[] = [
    { label: "Wallet", screen: walletOpen ? "wallet" : "home", active: screen === "home" || screen === "wallet" },
    { label: "Setup", screen: "storage", active: inSetup },
    { label: "Settings", screen: "settings", active: screen === "settings" },
    { label: "About", screen: "about", active: screen === "about" },
  ]

  return (
    <>
      <header className="grid h-12 shrink-0 grid-cols-[auto_1fr_auto] items-center gap-4 border-b border-slate-800 bg-[var(--app-chrome)] px-5">
        <button type="button" onClick={() => onNavigate("home")} aria-label="Ryo Wallet Next — Start"
          title="Independent project"
          className="flex w-fit items-center gap-2.5 rounded-lg text-left focus-visible:outline-2 focus-visible:outline-sky-400">
          <img src={ryoMark} alt="Ryo Currency symbol" className="size-7 shrink-0" />
          <span className="text-sm font-semibold tracking-tight">Ryo Wallet Next</span>
        </button>

        <nav aria-label="Main navigation" className="flex items-center justify-center gap-1">
          {walletOpen && screen === "wallet" ? ([["overview", "Wallet"], ["receive", "Receive"], ["send", "Send"], ["contacts", "Address Book"], ["history", "TX History"]] as const).map(([section, label]) =>
            <button type="button" key={section} onClick={() => onWalletSection(section)} aria-current={walletSection === section ? "page" : undefined}
              className={"rounded-md px-3 py-1.5 text-sm focus-visible:outline-2 focus-visible:outline-sky-400 " +
                (walletSection === section ? "bg-sky-400/10 font-medium text-sky-200" : "text-slate-400 hover:bg-slate-800 hover:text-slate-100")}>{label}</button>) : null}
          {sections.filter((item) => !(walletOpen && screen === "wallet" && item.label === "Wallet")).map((item) => (
            <button key={item.label} type="button" onClick={() => onNavigate(item.screen)}
              disabled={!canVisit(item.screen)} aria-current={item.active ? "page" : undefined}
              className={"rounded-md px-4 py-1.5 text-sm transition-colors focus-visible:outline-2 focus-visible:outline-sky-400 disabled:cursor-not-allowed disabled:opacity-40 " +
                (item.active ? "bg-sky-400/10 font-medium text-sky-200" : "text-slate-400 hover:bg-slate-800 hover:text-slate-100")}>
              {item.label}
              {item.screen === "about" && updateVersion ? <span className="ml-2 inline-block size-1.5 rounded-full bg-sky-300" aria-label="Update available" /> : null}
            </button>
          ))}
        </nav>

        <div className="flex items-center justify-end gap-4">
          <div className="flex items-center gap-2 whitespace-nowrap text-[11px]">
            <span className="text-slate-400">{networkLabel} · Preview</span>
            <span className="font-mono text-slate-500">{versionLabel ?? "Checking version…"}</span>
          </div>
          {comparison ? <div className="border-l border-slate-700 pl-4">{comparison}</div> : null}
        </div>
      </header>

      {inSetup ? (
        <nav aria-label="Setup navigation" className="flex h-8 shrink-0 items-center gap-2 border-b border-slate-800 bg-[var(--app-chrome)] px-5">
          {navigation.filter((item) => item.screen !== "home").map((item, index) => (
            <div key={item.screen} className="flex items-center gap-2">
              {index > 0 ? <span aria-hidden="true" className="px-2 text-xs text-slate-700">/</span> : null}
              <button type="button" onClick={() => onNavigate(item.screen)} disabled={!canVisit(item.screen)}
                aria-current={screen === item.screen ? "page" : undefined}
                className={"flex items-center gap-2 rounded-md px-3 py-1 text-xs transition-colors focus-visible:outline-2 focus-visible:outline-sky-400 disabled:cursor-not-allowed disabled:opacity-35 " +
                  (screen === item.screen ? "bg-slate-800 font-medium text-slate-100" : "text-slate-400 hover:bg-slate-800/60 hover:text-slate-200")}>
                <span className={"font-mono text-[10px] " + (screen === item.screen ? "text-sky-300" : "text-slate-500")}>{item.number}</span>
                {item.label}
              </button>
            </div>
          ))}
        </nav>
      ) : null}
    </>
  )
}
