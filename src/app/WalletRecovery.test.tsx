// @vitest-environment jsdom
import { act } from "react"
import { createRoot, type Root } from "react-dom/client"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { afterEach, beforeEach, expect, it, vi } from "vitest"
import { getWalletOverview } from "@/api/overview"
import { walletOperation, type Transaction } from "@/api/operations"
import { WalletDashboard } from "./WalletDashboard"

vi.mock("@/api/overview", () => ({ getWalletOverview: vi.fn() }))
vi.mock("@/api/operations", () => ({ walletOperation: vi.fn() }))

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
async function mount() {
  await act(async () => root.render(<QueryClientProvider client={client}>
    <WalletDashboard generation="7" section="overview" onSection={() => {}} onLock={() => {}} onRemoved={() => {}} locking={false} />
  </QueryClientProvider>))
  await act(async () => { await vi.advanceTimersByTimeAsync(1) })
}

it("recovers balances, history and name automatically after the runtime finishes scanning", async () => {
  vi.mocked(getWalletOverview).mockRejectedValueOnce("wallet RPC busy").mockResolvedValue(snapshot)
  const attempts = new Map<string, number>()
  vi.mocked(walletOperation).mockImplementation(async (_generation, operation) => {
    const count = attempts.get(operation.type) ?? 0
    attempts.set(operation.type, count + 1)
    if (count === 0) throw "wallet RPC busy"
    return (operation.type === "info" ? { name: "Recovered wallet" } : [transaction]) as never
  })
  await mount()
  expect(container.textContent).toContain("Balances will update automatically")
  expect(container.textContent).toContain("Transactions will update automatically")
  expect(container.textContent).not.toContain("locking and reopening")
  expect(container.querySelector('[role="alert"]')).toBeNull()
  await act(async () => { await vi.advanceTimersByTimeAsync(15_100) })
  expect(container.textContent).toContain("Recovered wallet")
  expect(container.textContent).toContain("5 RYO")
  expect(container.textContent).toContain(transaction.txid)
  expect(container.textContent).not.toContain("Loading transactions")
  expect(container.textContent).not.toContain("Wallet is scanning")
  expect(attempts.get("history")).toBe(2)
  expect([...attempts.keys()]).toEqual(["info", "history"])
})

it("labels retained balances and history as the last available data during scanning", async () => {
  client.setQueryData(["wallet-overview", "7"], snapshot)
  client.setQueryData(["wallet-operation", "7", "history"], [transaction])
  vi.mocked(getWalletOverview).mockRejectedValue("wallet RPC busy")
  vi.mocked(walletOperation).mockRejectedValue("wallet RPC busy")
  await mount()
  expect(container.textContent).toContain("Showing the last available balances")
  expect(container.textContent).toContain("Showing the last available history")
  expect(container.textContent).toContain("5 RYO")
  expect(container.textContent).toContain(transaction.txid)
  expect(container.querySelector('[role="alert"]')).toBeNull()
})
