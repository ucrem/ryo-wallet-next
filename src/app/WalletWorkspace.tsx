import { useEffect, useState, type FormEvent } from "react"
import { useQuery, useQueryClient } from "@tanstack/react-query"
import {
  acknowledgeBackup, createWallet, getBackupPhrase,
  listWallets, lockWallet, openWallet, restoreWallet, walletRuntimeReady,
  type WalletEntry,
} from "@/api/wallet"
import { challengePositions, verifyBackupWords } from "./backupChallenge"
import { Button } from "@/components/ui/button"
import { WalletDashboard } from "./WalletDashboard"
import type { WalletSection } from "@/api/operations"
import { usePreferences } from "@/lib/usePreferences"
import { isWeakPassword } from "@/lib/passwordStrength"

type Phase = "entry" | "backup" | "verify" | "open"

export function WalletWorkspace({ mode, activeWallet, sessionGeneration, onBack, onLocked, section = "overview", onSection = () => {} }: {
  mode: "create" | "restore" | "open"; activeWallet: WalletEntry | null; sessionGeneration: string | null;
  onBack: () => void; onLocked: () => void; section?: WalletSection; onSection?: (section: WalletSection) => void
}) {
  const queryClient = useQueryClient()
  const [phase, setPhase] = useState<Phase>(activeWallet ? activeWallet.backup_complete ? "open" : "backup" : "entry")
  const [walletId, setWalletId] = useState(activeWallet?.id ?? "")
  const [phrase, setPhrase] = useState<string | null>(null)
  const [answers, setAnswers] = useState<Record<number, string>>({})
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const preferences = usePreferences()
  const [weakWarning, setWeakWarning] = useState(false)
  const [weakAccepted, setWeakAccepted] = useState(false)
  const runtime = useQuery({ queryKey: ["wallet-runtime-ready"], queryFn: walletRuntimeReady })
  const wallets = useQuery({ queryKey: ["wallet-list"], queryFn: listWallets })
  const selectedId = walletId || wallets.data?.[0]?.id || ""

  useEffect(() => {
    if (!activeWallet || activeWallet.backup_complete || phase !== "backup" || phrase) return
    let cancelled = false
    void getBackupPhrase(activeWallet.id)
      .then((result) => { if (!cancelled) setPhrase(result.recovery_phrase) })
      .catch((cause: unknown) => { if (!cancelled) setError(String(cause)) })
    return () => { cancelled = true }
  }, [activeWallet, phase, phrase])

  async function refreshWalletState() {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["foundation-status"] }),
      queryClient.invalidateQueries({ queryKey: ["wallet-list"] }),
      queryClient.invalidateQueries({ queryKey: ["active-wallet"] }),
      queryClient.invalidateQueries({ queryKey: ["wallet-overview"] }),
    ])
  }

  async function submitCreate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (busy) return
    const form = event.currentTarget
    const values = new FormData(form)
    const password = String(values.get("password") ?? "")
    if (password !== values.get("confirmation")) {
      setError("Passwords do not match.")
      return
    }
    if (new TextEncoder().encode(password).length < 12) {
      setError("Use a password of at least 12 bytes.")
      return
    }
    if ((preferences.data?.notify_weak_password ?? true) && isWeakPassword(password) && !weakAccepted) {
      setWeakWarning(true); return
    }
    setError(null)
    setBusy(true)
    form.reset()
    try {
      const created = await createWallet(password)
      setWalletId(created.wallet_id)
      setPhrase(created.recovery_phrase)
      setPhase("backup")
      await refreshWalletState()
    } catch (cause) {
      setError(String(cause))
    } finally {
      setBusy(false)
    }
  }

  async function submitRestore(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (busy) return
    const form = event.currentTarget
    const values = new FormData(form)
    const password = String(values.get("password") ?? "")
    const seed = String(values.get("seed") ?? "")
    const height = String(values.get("height") ?? "").trim() || "0"
    if (password !== values.get("confirmation")) {
      setError("Passwords do not match.")
      return
    }
    if (new TextEncoder().encode(password).length < 12) {
      setError("Use a password of at least 12 bytes.")
      return
    }
    if ((preferences.data?.notify_weak_password ?? true) && isWeakPassword(password) && !weakAccepted) {
      setWeakWarning(true); return
    }
    if (!seed.trim()) {
      setError("Enter your recovery phrase.")
      return
    }
    if (!/^\d+$/.test(height)) {
      setError("Scan start height must be a non-negative whole number.")
      return
    }
    setError(null)
    setBusy(true)
    form.reset()
    try {
      const restored = await restoreWallet(password, seed, height)
      setWalletId(restored.wallet_id)
      setPhase("backup")
      await refreshWalletState()
      setPhrase((await getBackupPhrase(restored.wallet_id)).recovery_phrase)
    } catch (cause) {
      setError(String(cause))
    } finally {
      setBusy(false)
    }
  }

  async function submitOpen(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (busy || !selectedId) return
    const form = event.currentTarget
    const password = String(new FormData(form).get("password") ?? "")
    form.reset()
    setError(null)
    setBusy(true)
    try {
      await openWallet(selectedId, password)
      setWalletId(selectedId)
      const needsBackup = !wallets.data?.find((entry) => entry.id === selectedId)?.backup_complete
      setPhase(needsBackup ? "backup" : "open")
      await refreshWalletState()
      if (needsBackup) {
        setPhrase((await getBackupPhrase(selectedId)).recovery_phrase)
      }
    } catch (cause) {
      setError(String(cause))
    } finally {
      setBusy(false)
    }
  }

  async function confirmBackup(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!phrase || busy) return
    if (!verifyBackupWords(phrase, answers)) {
      setError("The words do not match. Check your paper backup and try again.")
      return
    }
    setBusy(true)
    setError(null)
    try {
      await acknowledgeBackup(walletId)
      setAnswers({})
      setPhrase(null)
      setPhase("open")
      await refreshWalletState()
    } catch (cause) {
      setError(String(cause))
    } finally {
      setBusy(false)
    }
  }

  async function lock() {
    if (busy) return
    setBusy(true)
    setError(null)
    try {
      await lockWallet()
      setPhrase(null)
      setAnswers({})
      setPhase("entry")
      await refreshWalletState()
      onLocked()
    } catch (cause) {
      setError(String(cause))
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="flex min-h-full flex-col">
      {phase !== "open" ? (
        <h1 className="mb-3 text-3xl font-semibold tracking-tight">
          {phase === "entry" ? mode === "create" ? "Create wallet" : mode === "restore" ? "Restore wallet" : "Open wallet" : "Back up your recovery phrase"}
        </h1>
      ) : null}

      {phase === "entry" ? (
        <>
          <p className="mt-3 text-sm leading-6 text-slate-400">
            {mode === "create" ? "A new wallet will be saved in the private data folder you selected."
              : mode === "restore" ? "Enter your recovery phrase to create a separate wallet in the private data folder. Your original wallet files are not changed."
                : "Choose a wallet previously created in this app, then enter its password."}
          </p>
          {runtime.data === false ? (
            <p className="mt-6 rounded-lg border border-amber-500/30 bg-amber-500/10 p-4 text-sm text-amber-100" role="status">
              The verified Linux wallet runtime is unavailable. Restart <code>pnpm tauri dev</code>
              and check its terminal output.
            </p>
          ) : null}
          {mode === "create" ? (
            <form onChange={(event) => { if (event.target instanceof HTMLInputElement && event.target.type === "password") { setWeakAccepted(false); setWeakWarning(false) } }} onSubmit={(event) => void submitCreate(event)} className="mt-6 grid gap-4 md:grid-cols-2">
              <PasswordField name="password" label="New wallet password" autoComplete="new-password" />
              <PasswordField name="confirmation" label="Confirm password" autoComplete="new-password" />
              <p className="text-xs text-slate-400 md:col-span-2">At least 12 bytes. Keep it separate from your recovery phrase.</p>
              {weakWarning ? <WeakPasswordNotice accepted={weakAccepted} onChange={setWeakAccepted} /> : null}
              <Button type="submit" disabled={busy || runtime.data !== true} className="justify-self-start bg-sky-400 text-slate-950 hover:bg-sky-300 md:col-span-2">
                {busy ? "Creating…" : "Create wallet"}
              </Button>
            </form>
          ) : mode === "restore" ? (
            <form onChange={(event) => { if (event.target instanceof HTMLInputElement && event.target.type === "password") { setWeakAccepted(false); setWeakWarning(false) } }} onSubmit={(event) => void submitRestore(event)} className="mt-6 grid gap-4 md:grid-cols-2">
              <label className="grid gap-2 text-sm md:col-span-2"><span>Recovery phrase</span>
                <textarea name="seed" required rows={4} autoComplete="off" autoCapitalize="off" spellCheck={false}
                  className="rounded-md border border-slate-600 bg-[var(--app-input)] px-3 py-2" />
              </label>
              <p className="text-xs text-slate-400 md:col-span-2">Enter all words in order, including any checksum word. Spaces and line breaks are normalized before restoration.</p>
              <PasswordField name="password" label="New wallet password" autoComplete="new-password" />
              <PasswordField name="confirmation" label="Confirm password" autoComplete="new-password" />
              <p className="text-xs text-slate-400 md:col-span-2">At least 12 bytes. Keep it separate from your recovery phrase.</p>
              {weakWarning ? <WeakPasswordNotice accepted={weakAccepted} onChange={setWeakAccepted} /> : null}
              <label className="grid gap-2 text-sm"><span>Scan start height (optional)</span>
                <input name="height" type="text" inputMode="numeric" pattern="[0-9]*" placeholder="0"
                  autoComplete="off" spellCheck={false}
                  className="rounded-md border border-slate-600 bg-[var(--app-input)] px-3 py-2" />
              </label>
              <p className="text-xs text-slate-400">Leave blank to use the height embedded in the phrase, if present. An explicit height may skip earlier transactions. A full scan from genesis can be requested from Wallet actions after restoration.</p>
              <Button type="submit" disabled={busy || runtime.data !== true} className="justify-self-start bg-sky-400 text-slate-950 hover:bg-sky-300 md:col-span-2">
                {busy ? "Restoring…" : "Restore wallet"}
              </Button>
            </form>
          ) : wallets.isError ? (
            <p className="mt-6 text-sm text-red-300" role="alert">Wallet list could not be read.</p>
          ) : wallets.data?.length === 0 ? (
            <p className="mt-6 text-sm text-slate-300">No app-owned wallets were found in this data folder.</p>
          ) : (
            <form onSubmit={(event) => void submitOpen(event)} className="mt-6 grid gap-4 md:grid-cols-2">
              <label className="grid gap-2 text-sm"><span>Wallet</span>
                <select value={selectedId} onChange={(event) => setWalletId(event.target.value)}
                  className="rounded-md border border-slate-600 bg-[var(--app-input)] px-3 py-2">
                  {wallets.data?.map((entry) => <option key={entry.id} value={entry.id}>
                    Wallet {entry.id.slice(0, 8)}{entry.backup_complete ? "" : " · backup pending"}
                  </option>)}
                </select>
              </label>
              <PasswordField name="password" label="Wallet password" autoComplete="current-password" />
              <Button type="submit" disabled={busy || runtime.data !== true || !selectedId} className="justify-self-start bg-sky-400 text-slate-950 hover:bg-sky-300 md:col-span-2">
                {busy ? "Opening…" : "Open wallet"}
              </Button>
            </form>
          )}
          <Button type="button" variant="outline" className="mt-4 self-start" onClick={onBack}>← Back to setup</Button>
        </>
      ) : null}

      {phase === "backup" && phrase ? (
        <>
          <p className="mt-3 max-w-2xl text-sm leading-6 text-slate-300">
            Write these words on paper in order and store them privately. Anyone with the words can recover the wallet.
            The app will ask for three words after you hide this view.
          </p>
          <div className="mt-6 grid grid-cols-3 gap-3 rounded-xl border border-sky-500/40 bg-[var(--app-surface)] p-5 sm:grid-cols-5" aria-label="Recovery phrase">
            {phrase.trim().split(/\s+/).map((word, index) => (
              <div key={index} className="min-w-0 text-sm"><span className="mr-2 font-mono text-slate-500">{index + 1}.</span>
                <span className="break-all font-medium text-slate-100">{word}</span></div>
            ))}
          </div>
          <p className="mt-4 text-xs text-amber-200">Do not capture, paste or share this screen. If you close the app now, the backup will still be pending when you reopen this wallet.</p>
          <Button type="button" className="mt-6 self-start bg-sky-400 text-slate-950 hover:bg-sky-300"
            onClick={() => { setError(null); setPhase("verify") }}>I wrote it down →</Button>
        </>
      ) : null}
      {phase === "backup" && !phrase ? <p className="mt-6 text-sm text-slate-400">Loading recovery phrase…</p> : null}

      {phase === "verify" && phrase ? (
        <form onSubmit={(event) => void confirmBackup(event)} className="mt-6 grid max-w-lg gap-4">
          <p className="text-sm text-slate-300">The phrase is hidden. Enter these words from your paper backup:</p>
          {challengePositions(phrase).map((index) => (
            <label key={index} className="grid gap-2 text-sm"><span>Word {index + 1}</span>
              <input type="text" value={answers[index] ?? ""} onChange={(event) => setAnswers({ ...answers, [index]: event.target.value })}
                autoComplete="off" autoCapitalize="off" spellCheck={false} required
                className="rounded-md border border-slate-600 bg-[var(--app-input)] px-3 py-2" />
            </label>
          ))}
          <div className="flex gap-3">
            <Button type="button" variant="outline" onClick={() => { setAnswers({}); setError(null); setPhase("backup") }}>Show phrase again</Button>
            <Button type="submit" disabled={busy} className="bg-sky-400 text-slate-950 hover:bg-sky-300">
              {busy ? "Saving…" : "Confirm backup"}
            </Button>
          </div>
        </form>
      ) : null}

      {(phase === "backup" || phase === "verify") ? (
        <Button type="button" variant="outline" className="mt-6 self-start" onClick={() => void lock()} disabled={busy}>
          Lock and finish backup later
        </Button>
      ) : null}

      {phase === "open" && sessionGeneration !== null ? (
        <WalletDashboard key={sessionGeneration} generation={sessionGeneration} section={section} onSection={onSection}
          onLock={() => void lock()} locking={busy} onRemoved={() => {
            setPhase("entry"); void refreshWalletState(); onLocked()
          }} />
      ) : null}

      {error ? <p className="mt-5 rounded-lg border border-red-500/40 bg-red-500/10 p-3 text-sm text-red-200" role="alert">{error}</p> : null}
    </div>
  )
}

function PasswordField({ name, label, autoComplete }: { name: string; label: string; autoComplete: string }) {
  return <label className="grid gap-2 text-sm"><span>{label}</span>
    <input name={name} type="password" autoComplete={autoComplete} required minLength={12}
      className="rounded-md border border-slate-600 bg-[var(--app-input)] px-3 py-2" />
  </label>
}

function WeakPasswordNotice({ accepted, onChange }: { accepted: boolean; onChange: (value: boolean) => void }) {
  return <div role="alert" className="rounded-lg border border-amber-500/40 bg-amber-500/5 p-3 text-sm text-amber-200 md:col-span-2">
    <p>This password may be easy to guess. Use a longer unique password, preferably at least 16 characters.</p>
    <label className="mt-2 flex items-center gap-2"><input type="checkbox" checked={accepted} onChange={(event) => onChange(event.target.checked)} />I understand and want to use this password.</label>
  </div>
}
