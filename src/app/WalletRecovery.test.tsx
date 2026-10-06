// @vitest-environment jsdom
import { act } from "react"
import { createRoot, type Root } from "react-dom/client"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { afterEach, beforeEach, expect, it, vi } from "vitest"
import { getWalletOverview } from "@/api/overview"
import { walletOperation, type Transaction } from "@/api/operations"
import { WalletDashboard } from "./WalletDashboard"
import { WalletStatusBar } from "./WalletStatusBar"
import { getWalletSyncStatus } from "@/api/wallet"

vi.mock("@/api/overview", () => ({ getWalletOverview: vi.fn() }))
vi.mock("@/api/operations", () => ({ walletOperation: vi.fn() }))
vi.mock("@/api/wallet", () => ({ getWalletSyncStatus: vi.fn() }))
vi.mock("@tauri-apps/api/event", () => ({ listen: async () => () => {} }))

let container: HTMLDivElement
let root: Root
let client: QueryClient
const snapshot = { session_generation: "7", primary_address: "disposable-address", total: { atomic: "5000000000" }, unlocked: { atomic: "5000000000" }, locked: { atomic: "0" }, multisig_import_needed: false }
const transaction: Transaction = { txid: "a".repeat(64), type: "in", amount: "5000000000", fee: "0", height: "100", timestamp: "0", payment_id: "", note: "", unlock_time: "0", address: "", double_spend_seen: false, destinations: [] }

beforeEach(() => {
  vi.resetAllMocks()
  vi.useFakeTimers()
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
  container = document.createElement("div")
  document.body.append(container)
  root = createRoot(container)
  client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity } } })
})
afterEach(async () => {
  await act(async () => root.unmount())
  client.clear()
  container.remove()
  vi.useRealTimers()
})
async function mount(withStatusBar = false) {
  await act(async () => root.render(<QueryClientProvider client={client}>
    <WalletDashboard generation="7" section="overview" onSection={() => {}} onLock={() => {}} onRemoved={() => {}} locking={false} />
    {withStatusBar ? <WalletStatusBar serviceState="open" sessionGeneration="7" node={null} root={null} /> : null}
  </QueryClientProvider>))
  await act(async () => { await vi.advanceTimersByTimeAsync(1) })
}

it("recovers balances, history and name automatically after the runtime finishes scanning", async () => {
  client.setQueryData(["wallet-list", "test-root", "mainnet"], [{ id: "a".repeat(32), name: null, backup_complete: true }])
  client.setQueryData(["active-wallet", "7"], { id: "a".repeat(32), name: null, backup_complete: true })
  vi.mocked(getWalletOverview).mockRejectedValueOnce("wallet RPC busy").mockResolvedValue(snapshot)
  const attempts = new Map<string, number>()
  vi.mocked(walletOperation).mockImplementation(async (_generation, operation) => {
    const count = attempts.get(operation.type) ?? 0
    attempts.set(operation.type, count + 1)
    if (count === 0) throw "wallet RPC busy"
    return (operation.type === "info" ? { name: "Recovered wallet" } : [transaction]) as never
  })
  await mount()
  expect(container.querySelector('[role="status"][aria-label*="Balances update automatically"]')).not.toBeNull()
  expect(container.querySelector('[role="status"][aria-label*="Transactions update automatically"]')).toBeNull()
  expect(container.textContent).not.toContain("update automatically")
  expect(container.textContent).not.toContain("locking and reopening")
  expect(container.querySelector('[role="alert"]')).toBeNull()
  expect(client.getQueryState(["wallet-list", "test-root", "mainnet"])!.isInvalidated).toBe(false)
  await act(async () => { await vi.advanceTimersByTimeAsync(15_100) })
  expect(container.textContent).toContain("Recovered wallet")
  expect(container.textContent).toContain("5 RYO")
  expect(container.textContent).toContain(transaction.txid)
  expect(container.textContent).not.toContain("Loading transactions")
  expect(container.querySelector('[role="status"]')).toBeNull()
  expect(attempts.get("history")).toBe(2)
  expect([...attempts.keys()]).toEqual(["info", "history"])
  expect(client.getQueryState(["wallet-list", "test-root", "mainnet"])!.isInvalidated).toBe(true)
  expect(client.getQueryState(["active-wallet", "7"])!.isInvalidated).toBe(true)
})

it("labels retained balances and refreshes recent history silently during scanning", async () => {
  client.setQueryData(["wallet-overview", "7"], snapshot)
  client.setQueryData(["wallet-operation", "7", "history"], [transaction])
  vi.mocked(getWalletOverview).mockRejectedValue("wallet RPC busy")
  vi.mocked(walletOperation).mockRejectedValue("wallet RPC busy")
  await mount()
  expect(container.querySelector('[role="status"][title*="Showing the last available balances"]')).not.toBeNull()
  expect(container.querySelector('[role="status"][title*="Showing the last available history"]')).toBeNull()
  expect(container.textContent).not.toContain("Showing the last available")
  expect(container.textContent).toContain("5 RYO")
  expect(container.textContent).toContain(transaction.txid)
  expect(container.querySelector('[role="alert"]')).toBeNull()
})

it("keeps recent history free of update badges while the footer reports wallet synchronization", async () => {
  const synced = { wallet_height: "100", daemon_height: "100", network_height: "100", node_reachable: true, node_ready: true, node_offline: false, node_untrusted: false, wallet_rpc_busy: false, wallet_rpc_available: true }
  client.setQueryData(["wallet-sync-status", "7"], synced)
  client.setQueryData(["wallet-operation", "7", "history"], [transaction])
  vi.mocked(getWalletSyncStatus).mockResolvedValue(synced)
  vi.mocked(getWalletOverview).mockResolvedValue(snapshot)
  let finishRefresh!: (entries: Transaction[]) => void
  const refreshed = new Promise<Transaction[]>((resolve) => { finishRefresh = resolve })
  let historyAttempts = 0
  vi.mocked(walletOperation).mockImplementation(async (_generation, operation) => {
    if (operation.type === "history") {
      if (historyAttempts++ === 0) throw "wallet RPC busy"
      return await refreshed as never
    }
    return { name: "Synced wallet" } as never
  })
  await mount(true)
  const walletStatus = () => container.querySelector('[aria-label="Wallet synchronization status"]')!.textContent
  const historyIndicator = () => container.querySelector('[role="status"][aria-label*="Transactions update automatically"]')
  expect(walletStatus()).toContain("Wallet synced")
  expect(walletStatus()).toContain("100.0%")
  expect(historyIndicator()).toBeNull()
  expect(container.textContent).not.toContain("Update pending")
  expect(container.textContent).not.toContain("Syncing")
  expect(container.textContent).toContain(transaction.txid)
  await act(async () => { await vi.advanceTimersByTimeAsync(15_100) })
  expect(historyIndicator()).toBeNull()
  expect(container.textContent).not.toContain("Updating")
  expect(walletStatus()).toContain("Wallet synced")
  await act(async () => { finishRefresh([transaction]); await vi.advanceTimersByTimeAsync(1) })
  expect(historyIndicator()).toBeNull()
  expect(walletStatus()).toContain("100.0%")
})
