import { useEffect, useState, type FormEvent } from "react"
import { useQuery, useQueryClient } from "@tanstack/react-query"
import { getWalletOverview } from "@/api/overview"
import {
  acknowledgeBackup, createWallet, getBackupPhrase, listWallets, lockWallet, openWallet,
  walletRuntimeReady,
  type WalletEntry,
} from "@/api/wallet"
import { challengePositions, verifyBackupWords } from "./backupChallenge"
import { Button } from "@/components/ui/button"

type Phase = "entry" | "backup" | "verify" | "open"

export function WalletWorkspace({ mode, activeWallet, onBack, onLocked }: {
  mode: "create" | "open"; activeWallet: WalletEntry | null; onBack: () => void; onLocked: () => void
}) {
  const queryClient = useQueryClient()
  const [phase, setPhase] = useState<Phase>(activeWallet ? activeWallet.backup_complete ? "open" : "backup" : "entry")
  const [walletId, setWalletId] = useState(activeWallet?.id ?? "")
  const [phrase, setPhrase] = useState<string | null>(null)
  const [answers, setAnswers] = useState<Record<number, string>>({})
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const runtime = useQuery({ queryKey: ["wallet-runtime-ready"], queryFn: walletRuntimeReady })
  const wallets = useQuery({ queryKey: ["wallet-list"], queryFn: listWallets })
  const overview = useQuery({
    queryKey: ["wallet-overview", walletId],
    queryFn: getWalletOverview,
    enabled: phase === "open",
    retry: 2,
    retryDelay: (attempt) => Math.min(1_000 * 2 ** attempt, 4_000),
    refetchInterval: phase === "open" ? 5_000 : false,
    refetchIntervalInBackground: false,
  })
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
      <p className="text-xs font-semibold tracking-[0.16em] text-sky-300">WALLET</p>
      <h1 className="mt-3 text-3xl font-semibold tracking-tight">
        {phase === "entry" ? mode === "create" ? "Create wallet" : "Open wallet"
          : phase === "open" ? "Wallet open" : "Back up your recovery phrase"}
      </h1>

      {phase === "entry" ? (
        <>
          <p className="mt-3 text-sm leading-6 text-slate-400">
            {mode === "create" ? "A new wallet will be saved in the private data folder you selected."
              : "Choose a wallet previously created in this app, then enter its password."}
          </p>
          {runtime.data === false ? (
            <p className="mt-6 rounded-lg border border-amber-500/30 bg-amber-500/10 p-4 text-sm text-amber-100" role="status">
              The verified Linux wallet runtime is unavailable. Restart <code>pnpm tauri dev</code>
              and check its terminal output.
            </p>
          ) : null}
          {mode === "create" ? (
            <form onSubmit={(event) => void submitCreate(event)} className="mt-6 grid max-w-lg gap-4">
              <PasswordField name="password" label="New wallet password" autoComplete="new-password" />
              <PasswordField name="confirmation" label="Confirm password" autoComplete="new-password" />
              <p className="text-xs text-slate-400">At least 12 bytes. Keep it separate from your recovery phrase.</p>
              <Button type="submit" disabled={busy || runtime.data !== true} className="bg-sky-400 text-slate-950 hover:bg-sky-300">
                {busy ? "Creating…" : "Create wallet"}
              </Button>
            </form>
          ) : wallets.isError ? (
            <p className="mt-6 text-sm text-red-300" role="alert">Wallet list could not be read.</p>
          ) : wallets.data?.length === 0 ? (
            <p className="mt-6 text-sm text-slate-300">No app-owned wallets were found in this data folder.</p>
          ) : (
            <form onSubmit={(event) => void submitOpen(event)} className="mt-6 grid max-w-lg gap-4">
              <label className="grid gap-2 text-sm"><span>Wallet</span>
                <select value={selectedId} onChange={(event) => setWalletId(event.target.value)}
                  className="rounded-md border border-slate-600 bg-[#0d141c] px-3 py-2">
                  {wallets.data?.map((entry) => <option key={entry.id} value={entry.id}>
                    Wallet {entry.id.slice(0, 8)}{entry.backup_complete ? "" : " · backup pending"}
                  </option>)}
                </select>
              </label>
              <PasswordField name="password" label="Wallet password" autoComplete="current-password" />
              <Button type="submit" disabled={busy || runtime.data !== true || !selectedId} className="bg-sky-400 text-slate-950 hover:bg-sky-300">
                {busy ? "Opening…" : "Open wallet"}
              </Button>
            </form>
          )}
          <Button type="button" variant="outline" className="mt-auto self-start" onClick={onBack}>← Back to setup</Button>
        </>
      ) : null}

      {phase === "backup" && phrase ? (
        <>
          <p className="mt-3 max-w-2xl text-sm leading-6 text-slate-300">
            Write these words on paper in order and store them privately. Anyone with the words can recover the wallet.
            The app will ask for three words after you hide this view.
          </p>
          <div className="mt-6 grid grid-cols-3 gap-3 rounded-xl border border-sky-500/40 bg-[#151d27] p-5 sm:grid-cols-5" aria-label="Recovery phrase">
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
                className="rounded-md border border-slate-600 bg-[#0d141c] px-3 py-2" />
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

      {phase === "open" ? (
        <>
          <p className="mt-3 text-sm text-slate-300">Your wallet is open. No transaction controls are available yet.</p>

          {overview.isFetching && !overview.isPending ? (
            <p className="mt-3 text-xs text-slate-500">
              Refreshing wallet data…
            </p>
          ) : null}
          {overview.isError ? (
            <div
              className="mt-6 rounded-lg border border-amber-500/30 bg-amber-500/10 p-4"
              role="status"
            >
              <p className="text-sm text-amber-100">
                The wallet is open, but node data is not available yet.
              </p>

              <p className="mt-2 text-xs leading-5 text-amber-200">
                The wallet may still be connecting or synchronizing with the configured node.
              </p>

              <Button
                type="button"
                variant="outline"
                className="mt-3"
                onClick={() => void overview.refetch()}
                disabled={overview.isFetching}
              >
                {overview.isFetching ? "Retrying…" : "Retry"}
              </Button>
            </div>
          ) : null}
          {overview.data ? (
            <section className="mt-6 rounded-xl border border-slate-700 bg-[#151d27] p-5" aria-label="Wallet overview">
              <p className="text-xs uppercase tracking-wide text-slate-400">Primary address</p>
              <p className="mt-2 break-all font-mono text-sm">{overview.data.primary_address}</p>
              <div className="mt-5 grid gap-4 sm:grid-cols-3">
                <div>
                  <p className="text-xs uppercase tracking-wide text-slate-400">
                    Total
                  </p>
                  <p className="mt-1 font-mono text-lg">
                    {formatAtomicRyo(overview.data.total.atomic)} RYO
                  </p>
                </div>

                <div>
                  <p className="text-xs uppercase tracking-wide text-slate-400">
                    Unlocked
                  </p>
                  <p className="mt-1 font-mono text-lg">
                    {formatAtomicRyo(overview.data.unlocked.atomic)} RYO
                  </p>
                </div>

                <div>
                  <p className="text-xs uppercase tracking-wide text-slate-400">
                    Locked
                  </p>
                  <p className="mt-1 font-mono text-lg">
                    {formatAtomicRyo(overview.data.locked.atomic)} RYO
                  </p>
                </div>
              </div>
            </section>
          ) : null}
          <Button type="button" variant="outline" className="mt-6 self-start" onClick={() => void lock()} disabled={busy}>
            {busy ? "Locking…" : "Lock wallet"}
          </Button>
        </>
      ) : null}

      {error ? <p className="mt-5 rounded-lg border border-red-500/40 bg-red-500/10 p-3 text-sm text-red-200" role="alert">{error}</p> : null}
    </div>
  )
}

function PasswordField({ name, label, autoComplete }: { name: string; label: string; autoComplete: string }) {
  return <label className="grid gap-2 text-sm"><span>{label}</span>
    <input name={name} type="password" autoComplete={autoComplete} required minLength={12}
      className="rounded-md border border-slate-600 bg-[#0d141c] px-3 py-2" />
  </label>
}

function formatAtomicRyo(atomic: string): string {
  const value = BigInt(atomic)
  const scale = 1_000_000_000n
  const whole = value / scale
  const fraction = (value % scale)
    .toString()
    .padStart(9, "0")
    .replace(/0+$/, "")

  return fraction ? `${whole}.${fraction}` : whole.toString()
}
