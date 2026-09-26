import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, it, vi } from "vitest"
import type { ActivityEntry } from "@/api/generated/ActivityEntry"
import type { ActivitySnapshot } from "@/api/generated/ActivitySnapshot"
import { Activity, ActivityView, copyActivityTxid, filterActivity, formatActivityTime, formatAtomicRyo, type ActivityFilter } from "./Activity"

const txid = "a".repeat(64)
const incoming: ActivityEntry = {
  txid,
  direction: "incoming",
  status: "confirmed",
  amount_atomic: "125000000000",
  fee_atomic: null,
  height: "123",
  timestamp: "1700000000",
  payment_id: null,
}
const outgoing: ActivityEntry = {
  ...incoming, txid: "b".repeat(64), direction: "outgoing", amount_atomic: "18500000000", fee_atomic: "10000000",
}
const pending: ActivityEntry = { ...outgoing, txid: "c".repeat(64), status: "pending", height: null }
const pool: ActivityEntry = { ...incoming, txid: "d".repeat(64), status: "pool", height: null }
const failed: ActivityEntry = { ...outgoing, txid: "e".repeat(64), status: "failed", height: null }
const snapshot: ActivitySnapshot = { transactions: [incoming, outgoing, pending, pool, failed], truncated: false, pool_unavailable: false }

function view(overrides: Partial<Parameters<typeof ActivityView>[0]> = {}): string {
  return renderToStaticMarkup(<ActivityView
    snapshot={snapshot} filter="all" onFilter={() => {}} selectedId={null} onSelect={() => {}}
    onCopy={() => {}} copyStatus="idle" refreshing={false} refreshError={false} onRefresh={() => {}}
    {...overrides}
  />)
}

function renderQuery(client: QueryClient): string {
  return renderToStaticMarkup(<QueryClientProvider client={client}><Activity sessionGeneration="1" /></QueryClientProvider>)
}

describe("read-only wallet activity", () => {
  it("shows loading, empty, and failed initial load distinctly", async () => {
    expect(renderQuery(new QueryClient())).toContain("Loading transaction history")
    const empty = new QueryClient()
    empty.setQueryData(["wallet-activity", "1"], { transactions: [], truncated: false, pool_unavailable: false })
    expect(renderQuery(empty)).toContain("No transactions yet")
    const failed = new QueryClient()
    await failed.prefetchQuery({ queryKey: ["wallet-activity", "1"], queryFn: async () => { throw new Error("unavailable") }, retry: false })
    expect(renderQuery(failed)).toContain("Transaction history could not be loaded")
  })

  it("labels received, sent, pending, pool, and failed entries explicitly", () => {
    const markup = view()
    expect(markup).toContain("↓ Received")
    expect(markup).toContain("+125 RYO")
    expect(markup).toContain("↑ Sent")
    expect(markup).toContain("−18.5 RYO")
    expect(markup).toContain("In pool · Pending")
    expect(markup).toContain("Failed · Funds were not sent")
    expect(markup).toContain("18.5 RYO attempted")
    expect(markup).not.toContain("−18.5 RYO attempted")
  })

  it("filters by direction and pending state without hiding failed sends from Sent", () => {
    expect(filterActivity(snapshot.transactions, "received").map((item) => item.txid)).toEqual([incoming.txid, pool.txid])
    expect(filterActivity(snapshot.transactions, "sent").map((item) => item.txid)).toEqual([outgoing.txid, pending.txid, failed.txid])
    expect(filterActivity(snapshot.transactions, "pending").map((item) => item.txid)).toEqual([pending.txid, pool.txid])
    for (const filter of ["all", "received", "sent", "pending"] as ActivityFilter[]) {
      expect(view({ filter })).toContain(`aria-pressed="true"`)
    }
  })

  it("shows full transaction details, copy control, and truncation warning", () => {
    const markup = view({ snapshot: { ...snapshot, truncated: true }, selectedId: `${outgoing.txid}:confirmed:outgoing:${outgoing.amount_atomic}:${outgoing.height}:${outgoing.timestamp}` })
    expect(markup).toContain("Showing the most recent 250 transactions")
    expect(markup).toContain("Transaction details")
    expect(markup).toContain(outgoing.txid)
    expect(markup).toContain("Fee")
    expect(markup).toContain("0.01 RYO")
    expect(markup).toContain("Copy transaction ID")
    expect(view({ copyStatus: "error", selectedId: `${incoming.txid}:confirmed:incoming:${incoming.amount_atomic}:${incoming.height}:${incoming.timestamp}` })).toContain("Could not copy transaction ID")
    expect(view({ selectedId: `${failed.txid}:failed:outgoing:${failed.amount_atomic}::${failed.timestamp}` })).toContain("Failed send attempt")
  })

  it("keeps prior data visible with a refresh warning and progress", () => {
    const markup = view({ refreshError: true, refreshing: true })
    expect(markup).toContain("Refreshing activity")
    expect(markup).toContain("Previously loaded transactions are shown")
    expect(markup).toContain("↓ Received")
  })

  it("marks offline pool data as incomplete rather than pretending the history is complete", () => {
    expect(view({ snapshot: { ...snapshot, pool_unavailable: true } })).toContain("Pool transactions could not be refreshed")
  })

  it("copies the full transaction ID and reports clipboard failure", async () => {
    const write = vi.fn(async () => {})
    expect(await copyActivityTxid(txid, write)).toBe(true)
    expect(write).toHaveBeenCalledWith(txid)
    expect(await copyActivityTxid(txid, async () => { throw new Error("clipboard denied") })).toBe(false)
  })

  it("formats exact atomic amounts and never invents a missing timestamp", () => {
    expect(formatAtomicRyo("1")).toBe("0.000000001")
    expect(formatAtomicRyo("18446744073709551615")).toBe("18446744073.709551615")
    expect(formatActivityTime(null)).toBe("Unknown time")
    expect(formatActivityTime("18446744073709551615")).toBe("Unknown time")
  })
})
