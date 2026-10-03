import { invoke } from "@tauri-apps/api/core"
import type { Preferences } from "./generated/Preferences"
import type { NodeConfig } from "./generated/NodeConfig"
import type { NodeMode } from "./generated/NodeMode"
import type { NodeOptions } from "./generated/NodeOptions"
import type { Network } from "./generated/Network"

export type { Preferences, NodeOptions }
export type GeneralSettings = { mode: NodeMode; network: Network; host: string; port: number; advanced: NodeOptions }
export const defaultPreferences: Preferences = { theme: "dark", idle_lock_seconds: 300, minimize_to_tray: false, launch_on_startup: false, notify_no_payment_id: true, notify_weak_password: true }
export const defaultNodeOptions: NodeOptions = { daemon_log_level: 0, wallet_log_level: 0, in_peers: -1, out_peers: -1, limit_rate_up: -1, limit_rate_down: -1, p2p_port: 0, rpc_port: 0, zmq_port: 0, wallet_rpc_port: 0, public_p2p: false }
export const getPreferences = () => invoke<Preferences>("app_preferences")
export const savePreferences = (preferences: Preferences) => invoke<Preferences>("save_app_preferences", { preferences })
export const saveGeneralSettings = (settings: GeneralSettings) => invoke<NodeConfig>("save_general_settings", { settings })
export const recordActivity = (sessionGeneration: string) => invoke<void>("app_activity", { sessionGeneration })
