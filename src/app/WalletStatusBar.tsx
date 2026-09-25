import { useEffect } from "react"
import { listen } from "@tauri-apps/api/event"
import { useQuery, useQueryClient } from "@tanstack/react-query"
import type { NodeConfig } from "@/api/generated/NodeConfig"
import { getWalletSyncStatus, type WalletSyncStatus } from "@/api/wallet"

const WALLET_SYNC_EVENT = "wallet-sync-status"

export function WalletStatusBar({
  serviceState,
  sessionGeneration,
  node,
}: {
  serviceState: string | null
  sessionGeneration: string | null
  node: NodeConfig | null
}) {
  const queryClient = useQueryClient()
  const sync = useQuery({
    queryKey: ["wallet-sync-status", sessionGeneration],
    queryFn: getWalletSyncStatus,
    enabled: serviceState === "open" && sessionGeneration !== null,
    retry: false,
    refetchInterval: false,
    refetchOnWindowFocus: false,
    staleTime: Infinity,
  })

  useEffect(() => {
    if (serviceState !== "open" || sessionGeneration === null) return

    let disposed = false
    let unlisten: (() => void) | undefined
    void listen<WalletSyncStatus>(WALLET_SYNC_EVENT, (event) => {
      if (!disposed) {
        queryClient.setQueryData(["wallet-sync-status", sessionGeneration], event.payload)
      }
    }).then((listener) => {
      if (disposed) listener()
      else unlisten = listener
    }).catch(() => {
      // The initial snapshot remains available if event registration fails.
    })

    return () => {
      disposed = true
      unlisten?.()
    }
  }, [queryClient, serviceState, sessionGeneration])

  const data = serviceState === "open" ? sync.data : undefined

  const progress = syncProgress(
    data?.wallet_height ?? null,
    data?.network_height ?? null,
  )

  const synced =
    data?.node_reachable === true &&
    data.wallet_height !== null &&
    data.network_height !== null &&
    syncProgress(data.wallet_height, data.network_height) === 100

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
          Wallet {formatHeight(data?.wallet_height ?? null)}
        </span>

        <span className="whitespace-nowrap font-mono text-[11px] text-slate-300">
          Chain {formatHeight(data?.network_height ?? null)}
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

function formatHeight(height: string | null): string {
  if (height === null) return "—"
  try {
    return new Intl.NumberFormat().format(BigInt(height))
  } catch {
    return "—"
  }
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
