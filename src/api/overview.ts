import { invoke } from "@tauri-apps/api/core"
import type { WalletOverview } from "@/api/generated/WalletOverview"

/** Scoped read of the app-owned, currently open wallet. */
export async function getWalletOverview(): Promise<WalletOverview> {
  return invoke<WalletOverview>("wallet_overview")
}
