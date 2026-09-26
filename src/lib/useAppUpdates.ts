import { useCallback, useEffect, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"

export type UpdateCheck =
  | { state: "development" }
  | { state: "current" }
  | { state: "available"; version: string; notes: string | null; automatic_install: boolean }

type UpdateProgress = { downloaded: number; total: number | null }

export type AppUpdates = {
  result: UpdateCheck | null
  checking: boolean
  installing: boolean
  checked: boolean
  error: string | null
  progress: UpdateProgress | null
  checkNow: () => Promise<void>
  install: (version: string) => Promise<void>
}

export function useAppUpdates(isNative: boolean): AppUpdates {
  const [result, setResult] = useState<UpdateCheck | null>(null)
  const [checking, setChecking] = useState(false)
  const [installing, setInstalling] = useState(false)
  const [checked, setChecked] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [progress, setProgress] = useState<UpdateProgress | null>(null)
  const checkingRef = useRef(false)
  const installingRef = useRef(false)
  const initialCheckRef = useRef(false)

  const checkUpdates = useCallback(async (manual: boolean) => {
    if (!isNative || checkingRef.current || installingRef.current) return
    checkingRef.current = true
    setChecking(true)
    setError(null)
    if (manual) setResult(null)
    const started = Date.now()
    try {
      setResult(await invoke<UpdateCheck>("app_update_check"))
    } catch {
      setError("Could not check for updates. Check your connection and try again.")
    } finally {
      if (manual) await new Promise((resolve) => setTimeout(resolve, Math.max(0, 450 - (Date.now() - started))))
      checkingRef.current = false
      setChecked(true)
      setChecking(false)
    }
  }, [isNative])

  useEffect(() => {
    if (!isNative || initialCheckRef.current) return
    initialCheckRef.current = true
    void checkUpdates(false)
  }, [checkUpdates, isNative])

  const install = useCallback(async (version: string) => {
    if (!isNative || installingRef.current || result?.state !== "available" ||
      !result.automatic_install || result.version !== version) return
    installingRef.current = true
    setInstalling(true)
    setProgress(null)
    setError(null)
    let unlisten: (() => void) | undefined
    try {
      unlisten = await listen<UpdateProgress>("app-update-progress", (event) => {
        setProgress(event.payload)
      })
      await invoke("app_update_install", { expectedVersion: version })
    } catch {
      setError("The update could not be installed. Your current app is still available; check again before retrying.")
    } finally {
      unlisten?.()
      installingRef.current = false
      setInstalling(false)
    }
  }, [isNative, result])

  return {
    result, checking, installing, checked, error, progress,
    checkNow: () => checkUpdates(true), install,
  }
}
