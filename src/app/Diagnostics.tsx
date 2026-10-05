import { useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { Button } from "@/components/ui/button"

export function Diagnostics({ isNative }: { isNative: boolean }) {
  const pending = useRef(false)
  const [saving, setSaving] = useState(false)
  const [result, setResult] = useState<"saved" | "cancelled" | "error" | null>(null)

  async function exportReport() {
    if (!isNative || pending.current) return
    pending.current = true
    setSaving(true)
    setResult(null)
    try {
      setResult(await invoke<boolean>("app_export_diagnostics") ? "saved" : "cancelled")
    } catch {
      setResult("error")
    } finally {
      pending.current = false
      setSaving(false)
    }
  }

  return (
    <section aria-labelledby="diagnostics-title" className="mt-5 rounded-xl border border-slate-700 bg-[var(--app-surface)] p-5 sm:p-6">
      <h2 id="diagnostics-title" className="text-xl font-semibold">Diagnostics</h2>
      <p className="mt-2 text-sm text-slate-400">Save the app version, node status and sync heights to help investigate a problem. The report excludes wallet secrets, addresses, names and file paths.</p>
      <Button type="button" variant="outline" className="mt-4" onClick={() => void exportReport()} disabled={!isNative || saving}>
        {saving ? "Saving…" : "Export diagnostic report"}
      </Button>
      {result === "error" ? <p className="mt-3 text-sm text-red-300" role="alert">Could not save the report. Choose a new JSON filename outside the app data folder and try again.</p> : result ? (
        <p className="mt-3 text-sm text-slate-300" role="status">{result === "saved" ? "Diagnostic report saved. Nothing was uploaded." : "Export cancelled."}</p>
      ) : null}
    </section>
  )
}
