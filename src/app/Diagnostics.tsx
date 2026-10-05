import { useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { Button } from "@/components/ui/button"

const exportErrors = {
  invalid_destination: "Choose a JSON filename in an existing folder.",
  protected_storage: "Choose a folder outside the application's private data folders.",
  file_exists: "A file with this name already exists. Choose a different filename.",
  destination_unavailable: "The selected folder is unavailable. Choose another location.",
  permission_denied: "The app cannot write to this location. Choose a writable folder.",
  storage_full: "There is not enough disk space to save the report. Choose another location.",
  create_failed: "Could not create the report file. Choose another folder and try again.",
  permissions_failed: "Could not apply private file permissions. Choose another folder.",
  write_failed: "Could not write the report. Please try again.",
  publish_failed: "Could not finish saving the report. Choose a different filename and try again.",
  file_picker_failed: "Could not open the save dialog. Please try again.",
} as const

function exportErrorMessage(error: unknown): string {
  if (typeof error === "object" && error !== null && "code" in error &&
      typeof error.code === "string" && Object.hasOwn(exportErrors, error.code)) {
    const message = exportErrors[error.code as keyof typeof exportErrors]
    const code = "os_error_code" in error ? error.os_error_code : null
    return message + (typeof code === "number" && Number.isInteger(code) && code > 0 && code <= 2_147_483_647
      ? ` (System error ${code}.)` : "")
  }
  return "Could not save the report. Please try again."
}

export function Diagnostics({ isNative }: { isNative: boolean }) {
  const pending = useRef(false)
  const [saving, setSaving] = useState(false)
  const [result, setResult] = useState<"saved" | "cancelled" | "error" | null>(null)
  const [errorMessage, setErrorMessage] = useState<string | null>(null)

  async function exportReport() {
    if (!isNative || pending.current) return
    pending.current = true
    setSaving(true)
    setResult(null)
    setErrorMessage(null)
    try {
      setResult(await invoke<boolean>("app_export_diagnostics") ? "saved" : "cancelled")
    } catch (error: unknown) {
      setErrorMessage(exportErrorMessage(error))
      setResult("error")
    } finally {
      pending.current = false
      setSaving(false)
    }
  }

  return (
    <section aria-labelledby="diagnostics-title" className="flex min-w-0 flex-col rounded-xl border border-slate-700 bg-[var(--app-surface)] p-4">
      <h2 id="diagnostics-title" className="text-lg font-semibold">Diagnostics</h2>
      <p className="mt-2 text-xs leading-5 text-slate-400">Export system, runtime and sync details for troubleshooting. Wallet secrets, addresses, names and file paths are excluded.</p>
      <div className="mt-auto pt-3">
        <Button type="button" variant="outline" size="sm" className="text-xs" onClick={() => void exportReport()} disabled={!isNative || saving}>
          {saving ? "Saving…" : "Export diagnostic report"}
        </Button>
      </div>
      {result === "error" ? <p className="mt-3 text-xs leading-5 text-red-300" role="alert">{errorMessage}</p> : result ? (
        <p className="mt-3 text-xs leading-5 text-slate-300" role="status">{result === "saved" ? "Diagnostic report saved. Nothing was uploaded." : "Export cancelled."}</p>
      ) : null}
    </section>
  )
}
