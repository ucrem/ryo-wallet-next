import { useState } from "react"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import type { NodeConfig } from "@/api/generated/NodeConfig"
import { getNodeConfiguration } from "@/api/node"
import { chooseDataRoot, getDataRootConfiguration } from "@/api/onboarding"
import { getWalletOverview } from "@/api/overview"
import { getFoundationStatus } from "@/api/status"
import { getActiveWallet } from "@/api/wallet"
import { NodeSetup } from "@/app/NodeSetup"
import { WalletWorkspace } from "@/app/WalletWorkspace"
import { About } from "@/app/About"
import ryoMark from "@/assets/ryo-mark.svg"
import { Button } from "@/components/ui/button"
import { useAppVersion } from "@/lib/useAppVersion"
import { WalletStatusBar } from "@/app/WalletStatusBar"

type WalletAction = "create" | "restore" | "open"
type Screen = "home" | "storage" | "node" | "summary" | "wallet" | "about"

const actionDetails: Record<WalletAction, { title: string; description: string }> = {
  create: { title: "Create a new wallet", description: "Set up a new private Ryo wallet." },
  restore: { title: "Restore a wallet", description: "Recover a wallet from its recovery phrase." },
  open: { title: "Open an existing wallet", description: "Import an existing wallet into private app storage." },
}

const navigation: { screen: Screen; label: string; number: string }[] = [
  { screen: "home", label: "Start", number: "◆" },
  { screen: "storage", label: "Data location", number: "01" },
  { screen: "node", label: "Node", number: "02" },
  { screen: "summary", label: "Summary", number: "03" },
  { screen: "wallet", label: "Wallet", number: "04" },
]

export function App() {
  const inDesktop = "__TAURI_INTERNALS__" in window
  const appVersion = useAppVersion()
  const queryClient = useQueryClient()
  const [screen, setScreen] = useState<Screen>("home")
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
      case "storage": return walletOpen || walletAction !== null
      case "node": return walletOpen || (walletAction !== null && root !== null && !dataRoot.isFetching)
      case "summary": return walletOpen || (walletAction !== null && root !== null && !!node.data && !node.isFetching)
      case "wallet": return walletOpen || (((walletAction === "create" || walletAction === "open") || !!activeWallet.data) && root !== null && !!node.data)
      case "about": return true
    }
  }

  return (
    <div className="flex h-dvh min-h-0 overflow-hidden bg-[#0c1118] text-slate-100">
      <aside className="flex w-56 shrink-0 flex-col border-r border-slate-800 bg-[#101720] p-5">
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
                (visibleScreen === item.screen ? "bg-sky-400/15 font-medium text-sky-200" : "text-slate-300 hover:bg-slate-800 hover:text-white")}
            >
              <span className="w-6 shrink-0 text-center font-mono text-xs text-slate-400">{item.number}</span>
              {item.label}
            </button>
          ))}
        </nav>
        <nav aria-label="Project navigation" className="mt-6 border-t border-slate-800 pt-5">
          <p className="mb-2 px-3 text-[11px] font-semibold uppercase tracking-[0.16em] text-slate-500">Project</p>
          <button type="button" onClick={() => setScreen("about")}
            aria-current={visibleScreen === "about" ? "page" : undefined}
            className={"flex h-11 w-full items-center gap-3 rounded-lg px-3 text-left text-sm transition-colors focus-visible:outline-2 focus-visible:outline-sky-400 " +
              (visibleScreen === "about" ? "bg-sky-400/15 font-medium text-sky-200" : "text-slate-300 hover:bg-slate-800 hover:text-white")}>
            <span aria-hidden="true" className="w-6 shrink-0 text-center text-base text-slate-400">ⓘ</span>
            About
          </button>
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
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-16 shrink-0 items-center justify-between border-b border-slate-800 px-8">
          <span className="text-sm font-medium text-slate-400">Ryo Wallet Next</span>
          <span className="rounded-full border border-slate-700 px-3 py-1 text-xs text-slate-300">Mainnet · Preview</span>
        </header>
        <main className="min-h-0 flex-1 overflow-y-auto px-8 py-8">
          <div className="mx-auto flex min-h-full max-w-3xl flex-col">
            {visibleScreen === "about" ? <About appVersion={appVersion} /> : null}
            {visibleScreen === "home" ? (
              <>
                <PageHeading eyebrow="GET STARTED" title="How would you like to use Ryo?"
                  description="Choose a wallet action. Storage and node settings follow in separate steps." />
                <div className="mt-7 grid gap-3">
                  {(["create", "restore", "open"] as const).map((action) => (
                    <button
                      key={action}
                      type="button"
                      onClick={() => start(action)}
                      className="group flex min-h-20 items-center justify-between gap-4 rounded-xl border border-slate-700 bg-[#151d27] px-5 py-4 text-left transition-colors hover:border-sky-500 hover:bg-[#1a2633] focus-visible:outline-2 focus-visible:outline-sky-400"
                    >
                      <span>
                        <span className="block font-medium text-slate-100">{actionDetails[action].title}</span>
                        <span className="mt-1 block text-sm text-slate-400">{actionDetails[action].description}</span>
                      </span>
                      <span aria-hidden="true" className="text-xl text-sky-300 group-hover:translate-x-1">→</span>
                    </button>
                  ))}
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
                <section className="mt-7 rounded-xl border border-slate-700 bg-[#151d27] p-5" aria-label="Data location">
                  <p className="text-sm font-medium">Wallet data folder</p>
                  <p className="mt-1 text-sm text-slate-400">Select a writable folder on this computer.</p>
                  {inDesktop && dataRoot.isPending ? <p className="mt-5 text-sm text-slate-400">Checking saved location…</p> : null}
                  {root ? (
                    <div className="mt-5 min-w-0 rounded-lg border border-slate-700 bg-[#0d141c] p-3" role="status">
                      <p className="text-xs font-medium uppercase tracking-wide text-slate-400">Selected path</p>
                      <p className="mt-1 break-all font-mono text-sm text-slate-100">{root}</p>
                    </div>
                  ) : null}
                  <Button type="button" className="mt-5 bg-sky-400 text-slate-950 hover:bg-sky-300"
                    onClick={() => { chooseRoot.reset(); chooseRoot.mutate() }}
                    disabled={!inDesktop || chooseRoot.isPending}>
                    {chooseRoot.isPending ? "Opening picker…" : root ? "Change folder" : "Choose folder"}
                  </Button>
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
                  description="Use your own local daemon or enter a remote node. This choice is saved for the selected data location." />
                <section className="mt-6 rounded-xl border border-slate-700 bg-[#151d27] p-5" aria-label="Node settings">
                  {!root ? (
                    <p className="text-sm text-slate-300">Choose a data location before selecting a node.</p>
                  ) : node.isPending ? (
                    <p className="text-sm text-slate-400">Loading node choice…</p>
                  ) : node.isError ? (
                    <p className="text-sm text-red-300" role="alert">The saved node choice could not be read. No settings were changed.</p>
                  ) : (
                    <NodeSetup key={root + ":" + node.data?.mode + ":" + node.data?.host + ":" + node.data?.port}
                      root={root} current={node.data ?? null} onSaved={() => setScreen("summary")} />
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
                <section className="mt-7 grid gap-4 rounded-xl border border-slate-700 bg-[#151d27] p-5" aria-label="Setup summary">
                  <SummaryRow label="Wallet action" value={actionDetails[walletAction ?? "open"].title} />
                  <SummaryRow label="Data location" value={root ?? "Not configured"} mono />
                  <SummaryRow label="Mainnet node" value={node.data ? nodeDescription(node.data) : "Not configured"} />
                </section>
                {walletAction === "restore" ? <p className="mt-5 rounded-lg border border-amber-500/30 bg-amber-500/10 p-4 text-sm text-amber-100">
                  Recovery from a phrase is still in development. No wallet has been changed.
                </p> : <p className="mt-5 text-sm text-slate-300">
                  Continue to {walletAction === "create" ? "create a new wallet and back up its recovery phrase" : "unlock an app-owned wallet"}.
                </p>}
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
                  {walletAction !== "restore" ? <Button type="button" className="bg-sky-400 text-slate-950 hover:bg-sky-300"
                    onClick={() => setScreen("wallet")}>Continue to wallet →</Button> : null}
                </div>
              </>
            ) : null}
            {visibleScreen === "wallet" && (status.data?.state === "open" || walletAction === "create" || walletAction === "open" || activeWallet.data) ? (
              status.data?.state === "open" && activeWallet.isPending ? (
                <p className="text-sm text-slate-400">Loading wallet…</p>
              ) : (
                <WalletWorkspace mode={walletAction === "create" ? "create" : "open"}
                  activeWallet={status.data?.state === "open" ? activeWallet.data ?? null : null}
                  sessionGeneration={status.data?.state === "open" ? status.data.session_generation : null}
                  onBack={() => setScreen("summary")} onLocked={() => setWalletAction("open")} />
              )
            ) : null}
          </div>
        </main>
        <WalletStatusBar
          serviceState={status.data?.state ?? null}
          sessionGeneration={status.data?.session_generation ?? null}
          node={node.data ?? null}
        />
      </div>
    </div>
  )
}

function PageHeading({ eyebrow, title, description }: { eyebrow: string; title: string; description: string }) {
  return (
    <div>
      <p className="text-xs font-semibold tracking-[0.16em] text-sky-300">{eyebrow}</p>
      <h1 className="mt-3 text-3xl font-semibold tracking-tight">{title}</h1>
      <p className="mt-3 max-w-2xl text-sm leading-6 text-slate-400">{description}</p>
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
  return node.mode === "local" ? "Local daemon on this computer" : node.host + ":" + node.port
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
