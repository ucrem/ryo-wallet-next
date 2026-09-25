import { invoke } from "@tauri-apps/api/core"
import type { LifecycleStatus } from "@/api/generated/LifecycleStatus"

export type WalletEntry = { id: string; backup_complete: boolean }
export type ReceiveAddress = {
  address_index: number
  address: string
  label: string
  used: boolean
}
export type CreatedWallet = {
  wallet_id: string
  status: LifecycleStatus
  recovery_phrase: string
}

export type WalletSyncStatus = {
  wallet_height: string | null
  daemon_height: string | null
  network_height: string | null
  node_reachable: boolean
  node_ready: boolean
  node_offline: boolean
  node_untrusted: boolean
}

export const getWalletSyncStatus = () =>
  invoke<WalletSyncStatus>("wallet_sync_status")

export const getReceiveAddresses = (sessionGeneration: string) =>
  invoke<ReceiveAddress[]>("wallet_receive_addresses", { sessionGeneration })
export const createReceiveAddress = (sessionGeneration: string) =>
  invoke<ReceiveAddress>("wallet_create_receive_address", { sessionGeneration })

export const walletRuntimeReady = () => invoke<boolean>("wallet_runtime_ready")
export const listWallets = () => invoke<WalletEntry[]>("wallet_list")
export const getActiveWallet = () => invoke<WalletEntry | null>("wallet_active")
export const createWallet = (password: string) => invoke<CreatedWallet>("wallet_create", { password })
export const openWallet = (walletId: string, password: string) =>
  invoke<LifecycleStatus>("wallet_open", { walletId, password })
export const lockWallet = () => invoke<LifecycleStatus>("wallet_lock")
export const getBackupPhrase = (walletId: string) =>
  invoke<{ recovery_phrase: string }>("wallet_backup_phrase", { walletId })
export const acknowledgeBackup = (walletId: string) =>
  invoke<void>("wallet_acknowledge_backup", { walletId })
