import { useState, useCallback } from "react"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import type { NodeConfig } from "@/api/generated/NodeConfig"
import { getNodeConfiguration } from "@/api/node"
import { chooseDataRoot, getDataRootConfiguration } from "@/api/onboarding"
import { getWalletOverview } from "@/api/overview"
import { getFoundationStatus } from "@/api/status"
import { getActiveWallet } from "@/api/wallet"
import { NodeSetup } from "@/app/NodeSetup"
import { WalletWorkspace } from "@/app/WalletWorkspace"
import { SavedWallets } from "@/app/SavedWallets"
import { About } from "@/app/About"
import { Settings } from "@/app/Settings"
import { useDesktopPreferences } from "@/lib/usePreferences"
import ryoMark from "@/assets/ryo-mark.svg"
import { Button } from "@/components/ui/button"
import { useAppVersion } from "@/lib/useAppVersion"
import { useAppUpdates } from "@/lib/useAppUpdates"
import { formatDataFolderPath } from "@/lib/displayPath"
import { WalletStatusBar } from "@/app/WalletStatusBar"
import { useNodeStatus } from "@/lib/useNodeStatus"
import { LayoutPreviewSwitch, TopNavigation } from "@/app/TopNavigation"
import { navigation, type NavigationLayout, type Screen } from "@/app/navigation"

import type { WalletSection } from "@/api/operations"

type WalletAction = "create" | "restore" | "open"

const actionDetails: Record<WalletAction, { title: string; description: string }> = {
  create: { title: "Create a new wallet", description: "Set up a new private Ryo wallet." },
  restore: { title: "Restore a wallet", description: "Recover a wallet from its recovery phrase." },
  open: { title: "Open an existing wallet", description: "Import an existing wallet into private app storage." },
}

export function App() {
  const layoutPreview = import.meta.env.MODE === "layout-preview"
  const [layout, setLayout] = useState<NavigationLayout>("top")
  const inDesktop = "__TAURI_INTERNALS__" in window
  const appVersion = useAppVersion()
  const updates = useAppUpdates(inDesktop)
  const queryClient = useQueryClient()
  const [screen, setScreen] = useState<Screen>("home")
  const [walletSection, setWalletSection] = useState<WalletSection>("overview")
  const [walletAction, setWalletAction] = useState<WalletAction | null>(null)
  const status = useQuery({
    queryKey: ["foundation-status"],
    queryFn: getFoundationStatus,
    enabled: inDesktop,
  })
  const activeWallet = useQuery({
    queryKey: ["active-wallet", status.data?.session_generation],
    queryFn: getActiveWallet,
    enabled: inDesktop && status.data?.state === "open",
  })
  const visibleScreen = screen
  const autoLocked = useCallback(() => {
    setWalletAction("open"); setWalletSection("overview"); setScreen((current) => current === "wallet" ? "home" : current)
  }, [])
  useDesktopPreferences(status.data?.state === "open" ? status.data.session_generation : null, autoLocked)
  const overview = useQuery({
    queryKey: ["wallet-overview", status.data?.session_generation],
    queryFn: getWalletOverview,
    enabled: inDesktop && status.data?.state === "open",
  })
  const dataRoot = useQuery({
    queryKey: ["data-root-configured"],
    queryFn: getDataRootConfiguration,
    enabled: inDesktop,
  })
  const root = dataRoot.data?.root ?? null
  const node = useQuery({
    queryKey: ["node-configuration", root],
    queryFn: getNodeConfiguration,
    enabled: inDesktop && root !== null,
  })
  const chain = useNodeStatus(node.data ?? null, root)
  const setupBusy = (!!status.data && !["locked", "stopped"].includes(status.data.state)) || (node.data?.mode !== "remote" && chain.data?.state === "running")
  const chooseRoot = useMutation({
    mutationFn: chooseDataRoot,
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["data-root-configured"] })
      void queryClient.invalidateQueries({ queryKey: ["node-configuration"] })
    },
  })

  function start(action: WalletAction) {
    setWalletAction(action)
    setScreen("storage")
  }

  function canVisit(destination: Screen): boolean {
    const walletOpen = status.data?.state === "open"
    switch (destination) {
      case "home": return true
      case "storage": return root !== null || walletOpen || walletAction !== null
      case "node": return root !== null || walletOpen || (walletAction !== null && root !== null && !dataRoot.isFetching)
      case "summary": return walletOpen || (walletAction !== null && root !== null && !!node.data && !node.isFetching)
      case "wallet": return walletOpen || (walletAction !== null && root !== null && !!node.data)
      case "about": return true
      case "settings": return true
    }
  }

  return (
    <div className="flex h-dvh min-h-0 overflow-hidden bg-[var(--app-bg)] text-slate-100">
      {layout === "sidebar" ? <aside className="flex w-56 shrink-0 flex-col border-r border-slate-800 bg-[var(--app-chrome)] p-5">
        <div className="flex items-center gap-3 border-b border-slate-800 pb-6">
          <img src={ryoMark} alt="Ryo Currency symbol" className="size-9 shrink-0" />
          <div className="min-w-0">
            <p className="truncate text-sm font-semibold leading-tight">Ryo Wallet Next</p>
            <p className="mt-1 text-xs text-slate-400">Independent project</p>
          </div>
        </div>
        <nav aria-label="Main navigation" className="mt-7 grid gap-1">
          <p className="mb-2 px-3 text-[11px] font-semibold uppercase tracking-[0.16em] text-slate-500">Navigation</p>
          {navigation.map((item) => (
            <button
              key={item.screen}
              type="button"
              onClick={() => setScreen(item.screen)}
              disabled={!canVisit(item.screen)}
              aria-current={visibleScreen === item.screen ? "page" : undefined}
              className={"flex h-11 items-center gap-3 rounded-lg px-3 text-left text-sm transition-colors focus-visible:outline-2 focus-visible:outline-sky-400 disabled:cursor-not-allowed disabled:opacity-40 " +
                (visibleScreen === item.screen ? "bg-sky-400/15 font-medium text-sky-200" : "text-slate-300 hover:bg-slate-800 hover:text-slate-100")}
            >
              <span className="w-6 shrink-0 text-center font-mono text-xs text-slate-400">{item.number}</span>
              {item.label}
            </button>
          ))}
        </nav>
        <nav aria-label="Project navigation" className="mt-6 border-t border-slate-800 pt-5">
          <p className="mb-2 px-3 text-[11px] font-semibold uppercase tracking-[0.16em] text-slate-500">Project</p>
          <button type="button" onClick={() => setScreen("settings")} className="flex h-11 w-full items-center rounded-lg px-3 text-sm text-slate-300 hover:bg-slate-800">Settings</button>
          <button type="button" onClick={() => setScreen("about")}
            aria-current={visibleScreen === "about" ? "page" : undefined}
            className={"flex h-11 w-full items-center gap-3 rounded-lg px-3 text-left text-sm transition-colors focus-visible:outline-2 focus-visible:outline-sky-400 " +
              (visibleScreen === "about" ? "bg-sky-400/15 font-medium text-sky-200" : "text-slate-300 hover:bg-slate-800 hover:text-slate-100")}>
            <span aria-hidden="true" className="w-6 shrink-0 text-center text-base text-slate-400">ⓘ</span>
            About
          </button>
          {updates.result?.state === "available" ? (
            <button type="button" onClick={() => setScreen("about")}
              className="mt-2 rounded-lg border border-sky-500/40 bg-sky-400/10 px-3 py-2 text-left text-xs text-sky-200 hover:bg-sky-400/20 focus-visible:outline-2 focus-visible:outline-sky-400">
              Update v{updates.result.version} available →
            </button>
          ) : null}
        </nav>
        <div className="mt-auto border-t border-slate-800 pt-5 text-xs text-slate-400">
          <p className="mb-4 font-mono text-slate-400">
            {appVersion.label ?? (appVersion.isLoading ? "Checking version…" : "Version unavailable")}
            {!appVersion.isNative ? " · browser preview" : ""}
          </p>
          <p className="font-medium text-slate-300">Native service</p>
          <p className="mt-1" role="status">
            {inDesktop
              ? status.isPending ? "Checking…" : status.isError ? "Unavailable" : status.data?.state ?? "Unavailable"
              : "Desktop app only"}
            {inDesktop && status.data ? " · session " + status.data.session_generation : ""}
          </p>
          <button
            type="button"
            className="mt-2 rounded text-sky-300 hover:text-sky-100 focus-visible:outline-2 focus-visible:outline-sky-400 disabled:opacity-40"
            onClick={() => void status.refetch()}
            disabled={!inDesktop || status.isFetching}
          >Check again</button>
        </div>
      </aside> : null}

      <div className="flex min-w-0 flex-1 flex-col">
        {layout === "top" ? <TopNavigation screen={visibleScreen} canVisit={canVisit} onNavigate={setScreen}
          walletOpen={status.data?.state === "open"} walletSection={walletSection} onWalletSection={(section) => { setWalletSection(section); setScreen("wallet") }} versionLabel={appVersion.label}
          updateVersion={updates.result?.state === "available" ? updates.result.version : null}
          networkLabel={(dataRoot.data?.network ?? "mainnet").replace(/^./, (letter) => letter.toUpperCase())}
          comparison={layoutPreview ? <LayoutPreviewSwitch layout={layout} onChange={setLayout} /> : undefined} />
          : <header className="flex h-16 shrink-0 items-center justify-between border-b border-slate-800 px-8">
          <span className="text-sm font-medium text-slate-400">Ryo Wallet Next</span>
          <div className="flex items-center gap-5">
            {layoutPreview ? <LayoutPreviewSwitch layout={layout} onChange={setLayout} /> : null}
            <span className="rounded-full border border-slate-700 px-3 py-1 text-xs text-slate-300">{(dataRoot.data?.network ?? "mainnet").replace(/^./, (letter) => letter.toUpperCase())} · Preview</span>
          </div>
        </header>}
        <main className="min-h-0 flex-1 overflow-y-auto px-5 py-5">
          <div className="flex min-h-full w-full flex-col">
            {visibleScreen === "about" ? <About appVersion={appVersion} updates={updates} /> : null}
            {visibleScreen === "settings" ? (root && node.isPending ? <p className="text-sm text-slate-400">Loading settings…</p> :
              <Settings key={`${root}:${dataRoot.data?.network ?? "mainnet"}`} root={root} node={node.data ?? null} network={dataRoot.data?.network ?? "mainnet"} busy={setupBusy} />) : null}
            {visibleScreen === "home" ? (
              <>
                <PageHeading eyebrow="GET STARTED" title="How would you like to use Ryo?"
                  description={root && node.data ? "Unlock a saved wallet, or choose an action to add another wallet." : "Choose a wallet action. Storage and node settings follow in separate steps."} />
                <div className="mt-5 grid gap-3 md:grid-cols-3">
                  <div className="grid content-start gap-3">
                    {inDesktop && root ? <SavedWallets key={`${root}:${dataRoot.data?.network ?? "mainnet"}`} root={root} network={dataRoot.data?.network ?? "mainnet"}
                      nodeConfigured={!!node.data} disabled={!status.data || !["locked", "stopped"].includes(status.data.state)}
                      onOpened={() => { setWalletAction("open"); setWalletSection("overview"); setScreen("wallet") }}
                      onConfigure={() => { setWalletAction("open"); setScreen("node") }} /> : null}
                    {(["create", "restore", "open"] as const).map((action) => (
                      <button
                        key={action}
                        type="button"
                        onClick={() => start(action)}
                        className="group flex min-h-16 items-center justify-between gap-4 rounded-xl border border-slate-700 bg-[var(--app-surface)] px-4 py-3 text-left transition-colors hover:border-sky-500 hover:bg-[var(--app-hover)] focus-visible:outline-2 focus-visible:outline-sky-400"
                      >
                        <span>
                          <span className="block font-medium text-slate-100">{actionDetails[action].title}</span>
                          <span className="mt-1 block text-sm text-slate-400">{actionDetails[action].description}</span>
                        </span>
                        <span aria-hidden="true" className="text-xl text-sky-300 group-hover:translate-x-1">→</span>
                      </button>
                    ))}
                  </div>
                  <div aria-hidden="true" className="pointer-events-none hidden items-center justify-center md:col-span-2 md:flex">
                    <img src={ryoMark} alt="" className="home-ryo-mark" />
                  </div>
                </div>
                <p className="mt-auto pt-6 text-xs text-slate-500">
                  An independent open-source project by ucrem. Wallet creation is available for local testing with a reviewed runtime.
                </p>
              </>
            ) : null}

            {visibleScreen === "storage" ? (
              <>
                <PageHeading eyebrow="SETUP · 1 OF 2" title="Choose a data location"
                  description={"For " + actionDetails[walletAction ?? "open"].title.toLowerCase() + ", choose where the app will keep its private wallet copy and runtime files."} />
                <section className="mt-7 rounded-xl border border-slate-700 bg-[var(--app-surface)] p-5" aria-label="Data location">
                  <p className="text-sm font-medium">Wallet data folder</p>
                  <p className="mt-1 text-sm text-slate-400">Select a writable folder on this computer.</p>
                  {inDesktop && dataRoot.isPending ? <p className="mt-5 text-sm text-slate-400">Checking saved location…</p> : null}
                  {root ? (
                    <div className="mt-5 min-w-0 rounded-lg border border-slate-700 bg-[var(--app-input)] p-3" role="status">
                      <p className="text-xs font-medium uppercase tracking-wide text-slate-400">Selected path</p>
                      <p className="mt-1 break-all font-mono text-sm text-slate-100">{formatDataFolderPath(root)}</p>
                    </div>
                  ) : null}
                  <Button type="button" className="mt-5 bg-sky-400 text-slate-950 hover:bg-sky-300"
                    onClick={() => { chooseRoot.reset(); chooseRoot.mutate() }}
                    disabled={!inDesktop || chooseRoot.isPending || setupBusy}>
                    {chooseRoot.isPending ? "Opening picker…" : root ? "Change folder" : "Choose folder"}
                  </Button>
                  {setupBusy ? <p className="mt-3 text-xs text-slate-400">Lock the wallet and stop the local node before changing the data folder.</p> : null}
                  {chooseRoot.isError || dataRoot.isError ? (
                    <p className="mt-3 text-sm text-red-300" role="alert">
                      The location could not be configured. Choose a writable folder and try again.
                    </p>
                  ) : null}
                  {chooseRoot.isSuccess && chooseRoot.data === false && !root ? (
                    <p className="mt-3 text-sm text-slate-400" role="status">No folder selected.</p>
                  ) : null}
                </section>
                <div className="mt-auto flex justify-between gap-3 pt-6">
                  <Button type="button" variant="outline" onClick={() => setScreen("home")}>Back</Button>
                  <Button type="button" className="bg-sky-400 text-slate-950 hover:bg-sky-300"
                    onClick={() => setScreen("node")} disabled={!canVisit("node") || chooseRoot.isPending}>
                    Continue to node →
                  </Button>
                </div>
              </>
            ) : null}

            {visibleScreen === "node" ? (
              <>
                <PageHeading eyebrow="SETUP · 2 OF 2" title="Choose a node"
                  description="Run the bundled local node or enter a remote node. The local node keeps syncing when you lock the wallet and stops when you close the app." />
                <section className="mt-6 rounded-xl border border-slate-700 bg-[var(--app-surface)] p-5" aria-label="Node settings">
                  {!root ? (
                    <p className="text-sm text-slate-300">Choose a data location before selecting a node.</p>
                  ) : node.isPending ? (
                    <p className="text-sm text-slate-400">Loading node choice…</p>
                  ) : node.isError ? (
                    <p className="text-sm text-red-300" role="alert">The saved node choice could not be read. No settings were changed.</p>
                  ) : (
                    <NodeSetup key={root + ":" + dataRoot.data?.network + ":" + node.data?.mode + ":" + node.data?.host + ":" + node.data?.port}
                      root={root} current={node.data ?? null} network={dataRoot.data?.network ?? "mainnet"} disabled={setupBusy} onSaved={() => {
                        if (walletAction === null) setWalletAction("open")
                        setScreen("summary")
                      }} />
                  )}
                </section>
                <div className="mt-auto pt-6">
                  <Button type="button" variant="outline" onClick={() => setScreen("storage")}>← Back to data location</Button>
                </div>
              </>
            ) : null}

            {visibleScreen === "summary" ? (
              <>
                <PageHeading eyebrow="SETUP SUMMARY" title="Your preferences are saved"
                  description="Review your setup before continuing." />
                <section className="mt-7 grid gap-4 rounded-xl border border-slate-700 bg-[var(--app-surface)] p-5" aria-label="Setup summary">
                  <SummaryRow label="Wallet action" value={actionDetails[walletAction ?? "open"].title} />
                  <SummaryRow label="Data location" value={root ? formatDataFolderPath(root) : "Not configured"} mono />
                  <SummaryRow label="Node" value={node.data ? `${node.data.network} · ${nodeDescription(node.data)}` : "Not configured"} />
                </section>
                <p className="mt-5 text-sm text-slate-300">
                  Continue to {walletAction === "create" ? "create a new wallet and back up its recovery phrase"
                    : walletAction === "restore" ? "restore from a recovery phrase and verify your backup"
                      : "unlock an app-owned wallet"}.
                </p>
                {overview.data ? (
                  <section className="mt-5 rounded-xl border border-slate-700 p-5 text-sm" aria-label="Wallet overview">
                    <p className="text-slate-400">Primary address</p>
                    <p className="mt-1 break-all font-mono text-xs">{overview.data.primary_address}</p>
                    <div className="mt-4 grid grid-cols-3 gap-3">
                      <Amount label="Total" atomic={overview.data.total.atomic} />
                      <Amount label="Unlocked" atomic={overview.data.unlocked.atomic} />
                      <Amount label="Locked" atomic={overview.data.locked.atomic} />
                    </div>
                  </section>
                ) : null}
                <div className="mt-auto flex justify-between gap-3 pt-6">
                  <Button type="button" variant="outline" onClick={() => setScreen("node")}>← Back to node</Button>
                  <Button type="button" className="bg-sky-400 text-slate-950 hover:bg-sky-300"
                    onClick={() => setScreen("wallet")}>Continue to wallet →</Button>
                </div>
              </>
            ) : null}
            {visibleScreen === "wallet" && (status.data?.state === "open" || walletAction !== null || activeWallet.data) ? (
              status.data?.state === "open" && activeWallet.isPending ? (
                <p className="text-sm text-slate-400">Loading wallet…</p>
              ) : (
                <WalletWorkspace section={walletSection} onSection={setWalletSection} mode={walletAction ?? "open"}
                  activeWallet={status.data?.state === "open" ? activeWallet.data ?? null : null}
                  sessionGeneration={status.data?.state === "open" ? status.data.session_generation : null}
                  onBack={() => setScreen("summary")} onLocked={() => {
                    setWalletAction("open"); setWalletSection("overview"); setScreen("home")
                    queryClient.removeQueries({ predicate: (query) => ["wallet-operation", "receive-addresses", "send-contact", "wallet-send-sync", "wallet-read-sync", "wallet-overview"].includes(String(query.queryKey[0])) })
                  }} />
              )
            ) : null}
          </div>
        </main>
        <WalletStatusBar
          serviceState={status.data?.state ?? null}
          sessionGeneration={status.data?.session_generation ?? null}
          node={node.data ?? null}
          root={root}
        />
      </div>
    </div>
  )
}

function PageHeading({ eyebrow, title, description }: { eyebrow: string; title: string; description: string }) {
  return (
    <div>
      <p className="text-xs font-semibold tracking-[0.16em] text-sky-300">{eyebrow}</p>
      <h1 className="mt-2 text-2xl font-semibold tracking-tight">{title}</h1>
      <p className="mt-2 text-sm leading-6 text-slate-400">{description}</p>
    </div>
  )
}

function SummaryRow({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="grid min-w-0 gap-1 border-b border-slate-700 pb-4 last:border-b-0 last:pb-0 sm:grid-cols-[9rem_1fr] sm:gap-3">
      <span className="text-sm text-slate-400">{label}</span>
      <span className={"min-w-0 break-all text-sm text-slate-100 " + (mono ? "font-mono" : "")}>{value}</span>
    </div>
  )
}

function nodeDescription(node: NodeConfig): string {
  return node.mode === "hybrid" ? `Local daemon with bootstrap ${node.bootstrap?.host}:${node.bootstrap?.port}` : node.mode === "local" ? "Local daemon on this computer" : node.host + ":" + node.port
}

function Amount({ label, atomic }: { label: string; atomic: string }) {
  return (
    <div>
      <p className="text-slate-400">{label}</p>
      <p className="mt-1 font-medium">{formatAtomicRyo(atomic)} RYO</p>
    </div>
  )
}

function formatAtomicRyo(atomic: string): string {
  const value = BigInt(atomic)
  const scale = 1_000_000_000n
  const whole = value / scale
  const fraction = (value % scale).toString().padStart(9, "0").replace(/0+$/, "")
  return fraction ? whole + "." + fraction : whole.toString()
}
