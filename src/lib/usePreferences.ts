import { useEffect } from "react"
import { useQuery, useQueryClient } from "@tanstack/react-query"
import { listen } from "@tauri-apps/api/event"
import { defaultPreferences, getPreferences, recordActivity, type Preferences } from "@/api/settings"
import type { LifecycleStatus } from "@/api/generated/LifecycleStatus"

export function usePreferences() {
  return useQuery({ queryKey: ["app-preferences"], queryFn: getPreferences, enabled: typeof window !== "undefined" && "__TAURI_INTERNALS__" in window, staleTime: Infinity })
}

export function useDesktopPreferences(generation: string | null, onLocked: () => void) {
  const client = useQueryClient()
  const query = usePreferences()
  const theme = query.data?.theme ?? defaultPreferences.theme
  useEffect(() => {
    const system = window.matchMedia("(prefers-color-scheme: light)")
    const apply = () => { document.documentElement.dataset.theme = theme === "system" ? system.matches ? "light" : "dark" : theme }
    apply(); system.addEventListener("change", apply)
    return () => system.removeEventListener("change", apply)
  }, [theme])
  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return
    let disposed = false
    const listeners: (() => void)[] = []
    const attach = (promise: Promise<() => void>) => void promise.then((unsubscribe) => { if (disposed) unsubscribe(); else listeners.push(unsubscribe) }).catch(() => {})
    attach(listen<Preferences>("preferences-changed", (event) => client.setQueryData(["app-preferences"], event.payload)))
    attach(listen<LifecycleStatus>("wallet-auto-locked", (event) => {
      client.setQueryData(["foundation-status"], event.payload)
      client.removeQueries({ predicate: (entry) => /^(wallet-|active-wallet|receive-addresses|send-contact)/.test(String(entry.queryKey[0])) })
      onLocked()
    }))
    return () => { disposed = true; listeners.forEach((unsubscribe) => unsubscribe()) }
  }, [client, onLocked])
  useEffect(() => {
    if (!generation || !("__TAURI_INTERNALS__" in window)) return
    let last = -Infinity
    const activity = () => {
      const now = performance.now()
      if (now - last < 5000) return
      last = now
      void recordActivity(generation).catch(() => {})
    }
    const events = ["pointermove", "pointerdown", "keydown", "wheel", "focus"] as const
    events.forEach((event) => window.addEventListener(event, activity, { passive: true }))
    activity()
    return () => events.forEach((event) => window.removeEventListener(event, activity))
  }, [generation])
}
