import { invoke } from "@tauri-apps/api/core"
import type { NodeConfig } from "@/api/generated/NodeConfig"

export type NodeSelection =
  | { mode: "local" }
  | { mode: "remote"; host: string; port: number }

export async function getNodeConfiguration(): Promise<NodeConfig | null> {
  return invoke<NodeConfig | null>("node_configuration")
}

export async function saveNodeSelection(selection: NodeSelection): Promise<NodeConfig> {
  return invoke<NodeConfig>("save_node_selection", { selection })
}
