import { useEffect, useState } from "react"
import { useQuery, useQueryClient } from "@tanstack/react-query"
import { writeText } from "@tauri-apps/plugin-clipboard-manager"
import type { ActivityEntry } from "@/api/generated/ActivityEntry"
import type { ActivitySnapshot } from "@/api/generated/ActivitySnapshot"
import { getWalletActivity } from "@/api/wallet"
import { Button } from "@/components/ui/button"

export type ActivityFilter = "all" | "received" | "sent" | "pending"

const filters: { value: ActivityFilter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "received", label: "Received" },
  { value: "sent", label: "Sent" },
  { value: "pending", label: "Pending" },
]

export function Activity({ sessionGeneration, onSessionChanged }: { sessionGeneration: string; onSessionChanged?: () => void }) {
  const queryClient = useQueryClient()
  const [filter, setFilter] = useState<ActivityFilter>("all")
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [copyStatus, setCopyStatus] = useState<"idle" | "copied" | "error">("idle")
  const key = ["wallet-activity", sessionGeneration] as const
  const activity = useQuery({
    queryKey: key,
    queryFn: () => getWalletActivity(sessionGeneration),
    refetchInterval: 15_000,
    refetchIntervalInBackground: false,
    retry: false,
    retryOnMount: false,
  })
  const staleSession = activity.error?.message === "wallet session changed; reopen the wallet page"

  useEffect(() => () => {
    const queryKey = ["wallet-activity", sessionGeneration]
    void queryClient.cancelQueries({ queryKey })
    queryClient.removeQueries({ queryKey })
  }, [queryClient, sessionGeneration])

  useEffect(() => {
    if (!staleSession) return
    const queryKey = ["wallet-activity", sessionGeneration]
    void queryClient.cancelQueries({ queryKey })
    queryClient.removeQueries({ queryKey })
    onSessionChanged?.()
  }, [queryClient, sessionGeneration, staleSession, onSessionChanged])

  async function copyTxid(txid: string) {
    setCopyStatus("idle")
    setCopyStatus(await copyActivityTxid(txid) ? "copied" : "error")
  }

  if (staleSession) {
    return <p role="alert" className="text-sm text-amber-200">The wallet session changed. Return to Wallet and open it again.</p>
  }
  if (!activity.data && activity.isPending) {
    return <p className="text-sm text-slate-300" role="status">Loading transaction history…</p>
  }
  if (!activity.data) {
    return <div role="alert" className="rounded-xl border border-red-500/40 bg-red-500/10 p-5 text-sm text-red-200">
      Transaction history could not be loaded. Keep the wallet open and try again.
      <Button type="button" variant="outline" className="ml-3" onClick={() => void activity.refetch()}>Retry</Button>
    </div>
  }

  return <ActivityView
    snapshot={activity.data}
    filter={filter}
    onFilter={setFilter}
    selectedId={selectedId}
    onSelect={(id) => { setSelectedId(id); setCopyStatus("idle") }}
    onCopy={copyTxid}
    copyStatus={copyStatus}
    refreshing={activity.isFetching}
    refreshError={activity.isError}
    onRefresh={() => void activity.refetch()}
  />
}

export async function copyActivityTxid(txid: string, write: (text: string) => Promise<void> = writeText): Promise<boolean> {
  try {
    await write(txid)
    return true
  } catch {
    return false
  }
}

export function ActivityView({ snapshot, filter, onFilter, selectedId, onSelect, onCopy, copyStatus, refreshing, refreshError, onRefresh }: {
  snapshot: ActivitySnapshot
  filter: ActivityFilter
  onFilter: (filter: ActivityFilter) => void
  selectedId: string | null
  onSelect: (id: string | null) => void
  onCopy: (txid: string) => void | Promise<void>
  copyStatus: "idle" | "copied" | "error"
  refreshing: boolean
  refreshError: boolean
  onRefresh: () => void
}) {
  const entries = filterActivity(snapshot.transactions, filter)
  const selected = snapshot.transactions.find((entry) => activityId(entry) === selectedId)
  return <div className="space-y-4 pb-6">
    <div className="flex flex-wrap items-center justify-between gap-3">
      <p className="text-sm text-slate-400">Read-only transaction history from the open wallet.</p>
      <Button type="button" variant="outline" disabled={refreshing} onClick={onRefresh}>
        {refreshing ? "Refreshing…" : "Refresh"}
      </Button>
    </div>
    {refreshing ? <p className="text-xs text-sky-200" role="status">Refreshing activity…</p> : null}
    {refreshError ? <p className="rounded-lg border border-amber-400/40 bg-amber-400/10 p-3 text-sm text-amber-100" role="alert">
      Refresh failed. Previously loaded transactions are shown; try again when the wallet RPC is available.
    </p> : null}
    {snapshot.truncated ? <p className="rounded-lg border border-amber-400/40 bg-amber-400/10 p-3 text-sm text-amber-100" role="status">
      Showing the most recent 250 transactions.
    </p> : null}
    {snapshot.pool_unavailable ? <p className="rounded-lg border border-amber-400/40 bg-amber-400/10 p-3 text-sm text-amber-100" role="status">
      Pool transactions could not be refreshed. Other history is shown; connect to the node and refresh to include the pool.
    </p> : null}
    <div role="group" aria-label="Activity filters" className="flex flex-wrap gap-2">
      {filters.map((item) => <button key={item.value} type="button" aria-pressed={filter === item.value}
        onClick={() => { onFilter(item.value); onSelect(null) }}
        className={"rounded-lg border px-3 py-1.5 text-sm focus-visible:outline-2 focus-visible:outline-sky-400 " +
          (filter === item.value ? "border-sky-400 bg-sky-400/15 text-sky-100" : "border-slate-700 text-slate-300 hover:border-slate-500")}>
        {item.label}
      </button>)}
    </div>
    {snapshot.transactions.length === 0 ? <p className="rounded-xl border border-slate-700 bg-[#151d27] p-5 text-sm text-slate-300">
      No transactions yet. Received and sent activity will appear here after the wallet finds it.
    </p> : entries.length === 0 ? <p className="rounded-xl border border-slate-700 bg-[#151d27] p-5 text-sm text-slate-300">
      No transactions match this filter.
    </p> : <div aria-label="Transactions" className="space-y-2">
      {entries.map((entry, index) => <button key={`${activityId(entry)}:${index}`} type="button" onClick={() => onSelect(activityId(entry))}
        aria-pressed={selectedId === activityId(entry)}
        className="flex w-full items-center justify-between gap-4 rounded-xl border border-slate-700 bg-[#151d27] p-4 text-left hover:border-sky-500 focus-visible:outline-2 focus-visible:outline-sky-400">
        <span className="min-w-0">
          <span className="block font-medium text-slate-100">{directionLabel(entry)}</span>
          <span className="mt-1 block text-xs text-slate-400">{statusLabel(entry)} · {formatActivityTime(entry.timestamp)}{entry.height ? ` · Block ${entry.height}` : ""}</span>
        </span>
        <span className="shrink-0 font-mono text-sm text-slate-100">{displayAmount(entry)}</span>
      </button>)}
    </div>}
    {selected ? <section aria-label="Transaction details" className="rounded-xl border border-sky-500/40 bg-[#151d27] p-5">
      <div className="flex items-start justify-between gap-3">
        <h2 className="text-lg font-semibold">Transaction details</h2>
        <Button type="button" variant="outline" onClick={() => onSelect(null)}>Close</Button>
      </div>
      <dl className="mt-4 grid gap-3 text-sm sm:grid-cols-[8rem_1fr]">
        <dt className="text-slate-400">Type</dt><dd>{selected.status === "failed" ? "Failed send attempt" : selected.direction === "incoming" ? "Received" : "Sent"}</dd>
        <dt className="text-slate-400">Status</dt><dd>{statusLabel(selected)}</dd>
        <dt className="text-slate-400">Amount</dt><dd className="font-mono">{displayAmount(selected)}</dd>
        {selected.fee_atomic ? <><dt className="text-slate-400">Fee</dt><dd className="font-mono">{formatAtomicRyo(selected.fee_atomic)} RYO</dd></> : null}
        <dt className="text-slate-400">Date</dt><dd>{formatActivityTime(selected.timestamp)}</dd>
        {selected.height ? <><dt className="text-slate-400">Block height</dt><dd className="font-mono">{selected.height}</dd></> : null}
        {selected.payment_id ? <><dt className="text-slate-400">Payment ID</dt><dd className="break-all font-mono">{selected.payment_id}</dd></> : null}
        <dt className="text-slate-400">Transaction ID</dt><dd className="min-w-0 break-all font-mono">{selected.txid}</dd>
      </dl>
      <Button type="button" variant="outline" className="mt-4" onClick={() => void onCopy(selected.txid)}>Copy transaction ID</Button>
      {copyStatus === "copied" ? <p className="mt-2 text-xs text-sky-200" role="status">Transaction ID copied.</p> : null}
      {copyStatus === "error" ? <p className="mt-2 text-xs text-red-300" role="alert">Could not copy transaction ID.</p> : null}
    </section> : null}
  </div>
}

export function filterActivity(entries: ActivityEntry[], filter: ActivityFilter): ActivityEntry[] {
  switch (filter) {
    case "all": return entries
    case "received": return entries.filter((entry) => entry.direction === "incoming")
    case "sent": return entries.filter((entry) => entry.direction === "outgoing")
    case "pending": return entries.filter((entry) => entry.status === "pending" || entry.status === "pool")
  }
}

function activityId(entry: ActivityEntry): string {
  return `${entry.txid}:${entry.status}:${entry.direction}:${entry.amount_atomic}:${entry.height ?? ""}:${entry.timestamp ?? ""}`
}

function directionLabel(entry: ActivityEntry): string {
  if (entry.status === "failed") return "↑ Failed send attempt"
  return entry.direction === "incoming" ? "↓ Received" : "↑ Sent"
}

function statusLabel(entry: ActivityEntry): string {
  switch (entry.status) {
    case "confirmed": return "Confirmed"
    case "pending": return "Pending"
    case "pool": return "In pool · Pending"
    case "failed": return "Failed · Funds were not sent"
  }
}

function displayAmount(entry: ActivityEntry): string {
  const amount = formatAtomicRyo(entry.amount_atomic)
  if (entry.status === "failed") return `${amount} RYO attempted`
  return `${entry.direction === "incoming" ? "+" : "−"}${amount} RYO`
}

export function formatAtomicRyo(atomic: string): string {
  try {
    const value = BigInt(atomic)
    const whole = value / 1_000_000_000n
    const fraction = (value % 1_000_000_000n).toString().padStart(9, "0").replace(/0+$/, "")
    return fraction ? `${whole}.${fraction}` : whole.toString()
  } catch {
    return "—"
  }
}

export function formatActivityTime(seconds: string | null): string {
  if (seconds === null) return "Unknown time"
  try {
    const milliseconds = BigInt(seconds) * 1_000n
    if (milliseconds > 8_640_000_000_000_000n) return "Unknown time"
    const date = new Date(Number(milliseconds))
    if (Number.isNaN(date.getTime())) return "Unknown time"
    return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(date)
  } catch {
    return "Unknown time"
  }
}
