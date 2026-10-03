import { useEffect } from "react"
import { listen } from "@tauri-apps/api/event"
import { useQuery, useQueryClient } from "@tanstack/react-query"
import type { NodeConfig } from "@/api/generated/NodeConfig"
import { getWalletSyncStatus, type WalletSyncStatus } from "@/api/wallet"
import { useNodeStatus } from "@/lib/useNodeStatus"
import { NodeControls } from "@/app/NodeControls"

const WALLET_SYNC_EVENT = "wallet-sync-status"

export function WalletStatusBar({
  serviceState,
  sessionGeneration,
  node,
  root,
}: {
  serviceState: string | null
  sessionGeneration: string | null
  node: NodeConfig | null
  root: string | null
}) {
  const queryClient = useQueryClient()
  const chain = useNodeStatus(node, root)
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
  const chainData = chain.isError ? undefined : chain.data
  const chainProgress = syncProgress(chainData?.height ?? null, chainData?.target_height ?? null)
  const chainLabel = !node ? "No node selected"
    : chain.isPending ? "Checking node"
      : node.mode !== "remote" && chainData?.state === "stopped" ? "Node stopped"
        : chainData?.state === "faulted" ? "Node needs restart"
          : !chainData?.reachable ? "Node unreachable"
            : chainData.offline ? "Node offline"
              : chainData.ready && chainProgress === 100 ? "Node synced"
                : chainData.target_height !== null ? "Node syncing" : "Finding peers"

  const synced =
    data?.node_reachable === true &&
    data.node_offline === false &&
    data.node_ready === true &&
    data.wallet_height !== null &&
    data.network_height !== null &&
    syncProgress(data.wallet_height, data.network_height) === 100

  let status = "Wallet locked"

  if (serviceState === "open") {
    if (sync.isPending) {
      status = "Checking wallet"
    } else if (!data?.node_reachable || data?.node_offline) {
      status = "Wallet waiting"
    } else if (synced) {
      status = "Wallet synced"
    } else if (data.wallet_height !== null && data.network_height !== null) {
      status = "Wallet syncing"
    } else if (data.wallet_height === null) {
      status = "Checking wallet"
    } else {
      status = "Waiting for node"
    }
  }

  return (
    <footer
      className="shrink-0 border-t border-slate-800 bg-[var(--app-chrome)] px-5"
      aria-label="Synchronization status"
    >
      <div className="flex h-10 items-center gap-4 whitespace-nowrap text-[11px] text-slate-400">
        <div className="flex shrink-0 items-center gap-2" aria-label="Node synchronization status"
          title={chainData?.state === "stopped" ? "Start the node to resume from your data folder."
            : chainData?.reachable && chainData.target_height === null ? "Chain loaded · finding peers" : undefined}>
          <span aria-hidden="true" className={"size-1.5 rounded-full " +
            (chainLabel === "Node synced" ? "bg-emerald-400" : chainData?.reachable ? "bg-amber-400" : "bg-slate-500")} />
          {node ? <span className="text-slate-500">{node.mode === "hybrid" ? "Local + Remote" : node.mode === "local" ? "Local" : "Remote"}</span> : null}
          <span className="font-medium text-slate-200">{chainLabel}</span>
          {chainData?.height !== null && chainData?.height !== undefined ? <>
            <span className="font-mono">Chain {formatHeight(chainData.height)} / {formatHeight(chainData.target_height)}</span>
            <span className="font-mono text-slate-300">{chainProgress !== null ? `${chainProgress.toFixed(1)}%` : "—"}</span>
            <SyncProgress label="Blockchain synchronization" value={chainProgress} color="bg-emerald-400" />
          </> : null}
        </div>

        <span aria-hidden="true" className="h-4 border-l border-slate-700" />

        <div className="flex shrink-0 items-center gap-2" aria-label="Wallet synchronization status"
          title={serviceState !== "open" && node?.mode !== "remote" && chainData?.state === "running"
            ? "Wallet locked · node stays active" : undefined}>
          <span aria-hidden="true" className={"size-1.5 rounded-full " +
            (synced ? "bg-emerald-400" : serviceState === "open" ? "bg-amber-400" : "bg-slate-500")} />
          <span className="font-medium text-slate-200">{status}</span>
          {serviceState === "open" ? <>
            <span className="font-mono">{formatHeight(data?.wallet_height ?? null)} / {formatHeight(data?.network_height ?? null)}</span>
            <span className="font-mono text-slate-300">{progress !== null ? `${progress.toFixed(1)}%` : "—"}</span>
            <SyncProgress label="Wallet synchronization" value={progress} color="bg-sky-400" />
          </> : null}
        </div>

        <div className="ml-auto shrink-0"><NodeControls compact node={node} running={chainData?.state === "running"}
          walletActive={serviceState !== null && serviceState !== "locked" && serviceState !== "stopped"} /></div>
      </div>
    </footer>
  )
}

function SyncProgress({ label, value, color }: { label: string; value: number | null; color: string }) {
  return <div className="h-1 w-12 shrink-0 overflow-hidden rounded-full bg-slate-800" role="progressbar" aria-label={label}
    aria-valuemin={0} aria-valuemax={100} aria-valuenow={value ?? undefined}>
    {value !== null ? <div className={"h-full transition-[width] duration-300 " + color} style={{ width: `${value}%` }} /> : null}
  </div>
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
