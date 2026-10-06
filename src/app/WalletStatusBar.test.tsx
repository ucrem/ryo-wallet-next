import { renderToStaticMarkup } from "react-dom/server"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { describe, expect, it } from "vitest"
import type { NodeConfig } from "@/api/generated/NodeConfig"
import type { NodeStatus } from "@/api/generated/NodeStatus"
import type { WalletSyncStatus } from "@/api/wallet"
import { WalletStatusBar } from "./WalletStatusBar"

const node: NodeConfig = { mode: "local", network: "mainnet", host: "127.0.0.1", port: 12211, trust: "managed_local" }
const root = "test-root"
function render(status: NodeStatus, serviceState = "locked", wallet?: WalletSyncStatus) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  client.setQueryData(["node-status", root, node], status)
  if (wallet) client.setQueryData(["wallet-sync-status", "2"], wallet)
  return renderToStaticMarkup(<QueryClientProvider client={client}>
    <WalletStatusBar serviceState={serviceState} sessionGeneration="2" node={node} root={root} />
  </QueryClientProvider>)
}

describe("independent node status while locked", () => {
  it("shows live chain progress and node stop without showing a wallet scan", () => {
    const markup = render({ mode: "local", state: "running", generation: "1", height: "250", target_height: "1000", reachable: true, ready: false, offline: false, untrusted: false })
    expect(markup).toContain("Wallet locked")
    expect(markup).toContain("Node syncing")
    expect(markup).toContain("25.0%")
    expect(markup).toContain("Stop node")
    expect(markup).toContain("node stays active")
    expect(markup).toContain('aria-label="Blockchain synchronization"')
    expect(markup).not.toContain('aria-label="Wallet synchronization" aria-valuemin="0" aria-valuemax="100" aria-valuenow="25"')
  })
  it("offers an explicit restart for a stopped or failed node", () => {
    const base: NodeStatus = { mode: "local", state: "stopped", generation: "2", height: null, target_height: null, reachable: false, ready: false, offline: true, untrusted: false }
    expect(render(base)).toContain("Node stopped")
    expect(render(base)).toContain("Start node")
    expect(render({ ...base, state: "faulted" })).toContain("Node needs restart")
  })
  it("requires locking before node control so an open wallet keeps its daemon endpoint", () => {
    const markup = render({ mode: "local", state: "running", generation: "1", height: "250", target_height: "1000", reachable: true, ready: false, offline: false, untrusted: false }, "open")
    expect(markup).toContain("Lock the wallet to control the node.")
    expect(markup).toMatch(/<button[^>]*disabled=""[^>]*>Stop node<\/button>/)
  })
  it("reports wallet scanning while a reachable node is still downloading the chain", () => {
    const markup = render({ mode: "local", state: "running", generation: "1", height: "250", target_height: "1000", reachable: true, ready: false, offline: false, untrusted: false }, "open", {
      wallet_height: "200", daemon_height: "250", network_height: "1000", node_reachable: true, node_ready: false, node_offline: false, node_untrusted: false,
    })
    expect(markup).toContain("Wallet syncing")
    expect(markup).toContain("Node syncing")
    expect(markup).toContain("20.0%")
    expect(markup).toContain("25.0%")
    expect(markup).not.toContain("Node connecting")
    expect(markup).toContain('aria-label="Wallet synchronization" aria-valuemin="0" aria-valuemax="100" aria-valuenow="20"')
  })
  it("waits for node readiness before reporting a fully synced wallet", () => {
    const status: NodeStatus = { mode: "local", state: "running", generation: "1", height: "1000", target_height: "1000", reachable: true, ready: false, offline: false, untrusted: false }
    const wallet: WalletSyncStatus = { wallet_height: "1000", daemon_height: "1000", network_height: "1000", node_reachable: true, node_ready: false, node_offline: false, node_untrusted: false }
    expect(render(status, "open", wallet)).not.toContain("Wallet synced")
    expect(render({ ...status, ready: true }, "open", { ...wallet, node_ready: true })).toContain("Wallet synced")
  })
  it("keeps last reported node heights visible during a delayed reply without claiming readiness", () => {
    const markup = render({ mode: "local", state: "running", generation: "1", height: "250", target_height: "1000", reachable: false, ready: false, offline: true, untrusted: false })
    expect(markup).toContain("Node response delayed")
    expect(markup).toContain("Last reported chain height")
    expect(markup).toContain("25.0%")
    expect(markup).not.toContain("Node synced")
  })
  it("shows processed wallet progress while RPC is busy and never reports synced from it", () => {
    const status: NodeStatus = { mode: "local", state: "running", generation: "1", height: "1000", target_height: "1000", reachable: true, ready: true, offline: false, untrusted: false }
    const wallet: WalletSyncStatus = { wallet_height: "200", wallet_rpc_busy: true, daemon_height: "1000", network_height: "1000", node_reachable: true, node_ready: true, node_offline: false, node_untrusted: false }
    const markup = render(status, "open", wallet)
    expect(markup).toContain("Wallet scanning")
    expect(markup).toContain("20.0%")
    expect(markup).not.toContain("Checking wallet")
    expect(render(status, "open", { ...wallet, wallet_height: "1000" })).not.toContain("Wallet synced")
    expect(render(status, "open", { ...wallet, wallet_height: null })).toContain("Wallet busy")
    const unavailable = render(status, "open", { ...wallet, wallet_height: "1000", wallet_rpc_busy: false, wallet_rpc_available: false })
    expect(unavailable).toContain("Wallet status unavailable")
    expect(unavailable).not.toContain("Wallet synced")
  })
})
