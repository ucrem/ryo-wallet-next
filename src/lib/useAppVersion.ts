import { useQuery } from "@tanstack/react-query"
import { getVersion } from "@tauri-apps/api/app"
import { bundledChangelog } from "@/lib/bundledChangelog"
import { displayVersion, releaseStatus } from "@/lib/appVersion"

export type AppVersionInfo = {
  version: string | null
  label: string | null
  status: string | null
  isNative: boolean
  isLoading: boolean
  isError: boolean
}

export function useAppVersion(): AppVersionInfo {
  const isNative = "__TAURI_INTERNALS__" in window
  const nativeVersion = useQuery({
    queryKey: ["application-version"],
    queryFn: getVersion,
    enabled: isNative,
    staleTime: Infinity,
    retry: false,
  })
  const version = isNative ? nativeVersion.data ?? null : bundledChangelog[0].version

  return {
    version,
    label: version ? displayVersion(version) : null,
    status: version ? releaseStatus(version) : null,
    isNative,
    isLoading: isNative && nativeVersion.isPending,
    isError: isNative && nativeVersion.isError,
  }
}
