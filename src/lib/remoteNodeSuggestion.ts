import type { Network } from "@/api/generated/Network"

// RPC endpoint from the official Ryo Atom defaults; P2P seeds are not wallet RPC endpoints.
const mainnetSuggestion = {
  host: "wallet-node.ryo-currency.com",
  port: 12211,
  source: "https://github.com/ryo-currency/ryo-wallet/blob/6c8d0aa68245271fe0e781084b38583abf758869/src-electron/main-process/modules/backend.js#L54-L57",
}

export function remoteNodeSuggestion(network: Network) {
  return network === "mainnet" ? mainnetSuggestion : null
}

export function defaultNodeRpcPort(network: Network) {
  return network === "mainnet" ? 12211 : network === "testnet" ? 13311 : 14411
}
