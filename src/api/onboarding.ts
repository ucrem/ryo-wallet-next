import { invoke } from "@tauri-apps/api/core"

export type DataRootConfiguration = {
  root: string | null
}

/** The native backend owns the selected path and exposes it only for display. */
export async function getDataRootConfiguration(): Promise<DataRootConfiguration> {
  return invoke<DataRootConfiguration>("data_root_configuration")
}

/** Opens the native folder picker. False means that the picker was cancelled. */
export async function chooseDataRoot(): Promise<boolean> {
  return invoke<boolean>("choose_data_root")
}
