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
    } else if (!data?.node_reachable || data.node_offline) {
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
      className="flex h-12 shrink-0 items-center gap-5 border-t border-slate-800 bg-[#101720] px-6 text-xs text-slate-400"
      aria-label="Wallet synchronization status"
    >
      <div className="flex min-w-24 items-center gap-2">
        <span
          aria-hidden="true"
          className={
            "size-2 rounded-full " +
            (status === "Synced"
              ? "bg-emerald-400"
              : status === "Syncing" || status === "Connecting"
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

      <div className="flex min-w-0 flex-1 items-center gap-3">
        <div
          className="h-1.5 min-w-20 flex-1 overflow-hidden rounded-full bg-slate-800"
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

        <span className="w-12 text-right font-mono">
          {progress !== null ? `${progress.toFixed(1)}%` : "—"}
        </span>
      </div>

      <div className="hidden whitespace-nowrap font-mono text-[11px] xl:block">
        Wallet {data?.wallet_height ?? "—"}
        {" / "}
        Network {data?.network_height ?? "—"}
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