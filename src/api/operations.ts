import { invoke } from "@tauri-apps/api/core"

export type WalletSection = "overview" | "receive" | "send" | "contacts" | "history"
export type Contact = { id: string; name: string; address: string; payment_id: string; notes: string }
export type Transaction = {
  txid: string; type: "in" | "out" | "pending" | "failed" | "pool"; amount: string; fee: string
  height: string; timestamp: string; payment_id: string; note: string; unlock_time: string
  address: string; double_spend_seen: boolean; destinations: { address: string; amount: string }[]
}
export type SendDraft = { token: string; address: string; payment_id: string; amount: string; fee: string; total: string; transactions: string[]; expires_in_seconds: number }
export type SendEntry = { txid: string; state: "not_sent" | "unknown" | "relayed" | "confirmed" | "failed" }
export type SecretMaterial = { phrase: string; view_key: string; spend_key: string }
export type AddressBalance = { index: number; balance: string; unlocked: string; outputs: string }
export type WalletOperation =
  | { type: "info" | "history" | "contacts" | "address_balances" | "send_journal" }
  | { type: "set_name"; name: string }
  | { type: "set_note"; txid: string; note: string }
  | { type: "save_contact"; id: string | null; name: string; address: string; payment_id: string; notes: string }
  | { type: "delete_contact"; id: string }
  | { type: "label_address"; index: number; label: string }
  | { type: "make_request"; address: string; amount: string; payment_id: string; description: string }
  | { type: "parse_request"; uri: string }
  | { type: "prepare_send"; address: string; amount: string; sweep: boolean; payment_id: string; priority: number; ring_size: number }
  | { type: "confirm_send" | "cancel_send"; token: string }
  | { type: "change_password"; old_password: string; new_password: string }
  | { type: "rescan"; spent_only: boolean }
  | { type: "secrets"; password: string }

export const walletOperation = <T = Record<string, never>>(sessionGeneration: string, operation: WalletOperation) =>
  invoke<T>("wallet_operation", { sessionGeneration, operation })
export const manageKeyImages = (sessionGeneration: string, importFile: boolean, password: string) =>
  invoke<unknown | null>("wallet_key_images", { sessionGeneration, import: importFile, password })
export const removeWallet = (sessionGeneration: string, password: string, confirmation: string) =>
  invoke<void>("wallet_remove", { sessionGeneration, password, confirmation })
export type Artwork = { size: number; cells: [number, number][]; color: string }
export const exportArtwork = (artwork: Artwork) => invoke<boolean>("wallet_export_artwork", artwork)
