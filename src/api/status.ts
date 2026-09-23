import { invoke } from "@tauri-apps/api/core"
import type { LifecycleStatus } from "@/api/generated/LifecycleStatus"

/** One explicit native command; no generic RPC or process bridge. */
export async function getFoundationStatus(): Promise<LifecycleStatus> {
  return invoke<LifecycleStatus>("app_status")
}
