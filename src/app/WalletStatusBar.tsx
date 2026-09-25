import { useQuery } from "@tanstack/react-query"
import type { NodeConfig } from "@/api/generated/NodeConfig"
import { getWalletSyncStatus } from "@/api/wallet"

export function WalletStatusBar({
  serviceState,
  node,
}: {
  serviceState: string | null
  node: NodeConfig | null
}) {
  const sync = useQuery({
    queryKey: ["wallet-sync-status"],
    queryFn: getWalletSyncStatus,
    enabled: serviceState === "open",
    retry: false,
    refetchInterval: serviceState === "open" ? 3_000 : false,
    refetchIntervalInBackground: false,
    staleTime: 1_000,
  })

  const data = serviceState === "open" ? sync.data : undefined

  const progress = syncProgress(
    data?.wallet_height ?? null,
    data?.network_height ?? null,
  )

  const synced =
    data?.node_reachable === true &&
    data.wallet_height !== null &&
    data.network_height !== null &&
    BigInt(data.wallet_height) >= BigInt(data.network_height)

  let status = "Wallet locked"

  if (serviceState === "open") {
    if (sync.isPending) {
      status = "Connecting"
    } else if (!data?.node_reachable || data?.node_offline) {
      status = "Node offline"
    } else if (!data.node_ready) {
      status = "Node connecting"
    } else if (synced) {
      status = "Synced"
    } else if (data.wallet_height !== null && data.network_height !== null) {
      status = "Syncing"
    } else {
      status = "Connecting"
    }
  }

  return (
    <footer
      className="shrink-0 border-t border-slate-800 bg-[#101720] px-6 py-3"
      aria-label="Wallet synchronization status"
    >
      <div className="flex flex-wrap items-center gap-x-5 gap-y-2 text-xs text-slate-400">
        <div className="flex min-w-24 items-center gap-2">
          <span
            aria-hidden="true"
            className={
              "size-2 rounded-full " +
              (status === "Synced"
                ? "bg-emerald-400"
                : status === "Syncing" || status === "Connecting" || status === "Node connecting"
                  ? "bg-amber-400"
                  : "bg-slate-500")
            }
          />
          <span className="font-medium text-slate-200">{status}</span>
        </div>

        <span className="whitespace-nowrap">
          {node?.mode === "remote"
            ? "Remote node"
            : node?.mode === "local"
              ? "Local node"
              : "No node"}
        </span>

        <span className="whitespace-nowrap font-mono text-[11px] text-slate-300">
          Wallet {data?.wallet_height ?? "—"}
        </span>

        <span className="whitespace-nowrap font-mono text-[11px] text-slate-300">
          Chain {data?.network_height ?? "—"}
        </span>

        <span className="ml-auto w-16 text-right font-mono text-slate-200">
          {progress !== null ? `${progress.toFixed(1)}%` : "—"}
        </span>
      </div>

      <div
        className="mt-2 h-1.5 overflow-hidden rounded-full bg-slate-800"
        role="progressbar"
        aria-label="Wallet synchronization"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={progress ?? undefined}
      >
        {progress !== null ? (
          <div
            className="h-full bg-sky-400 transition-[width] duration-300"
            style={{ width: `${progress}%` }}
          />
        ) : null}
      </div>
    </footer>
  )
}

function syncProgress(
  walletHeight: string | null,
  networkHeight: string | null,
): number | null {
  if (!walletHeight || !networkHeight) return null

  try {
    const wallet = BigInt(walletHeight)
    const network = BigInt(networkHeight)

    if (network <= 0n) return null
    if (wallet >= network) return 100

    const tenths = (wallet * 1000n) / network
    return Number(tenths) / 10
  } catch {
    return null
  }
}