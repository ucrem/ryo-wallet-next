import { useRef, useState, type FormEvent } from "react"
import { useQuery, useQueryClient } from "@tanstack/react-query"
import type { Network } from "@/api/generated/Network"
import { listWallets, openWallet, walletRuntimeReady } from "@/api/wallet"
import { Button } from "@/components/ui/button"

export function SavedWallets({ root, network, disabled, nodeConfigured, onOpened, onConfigure }: {
  root: string; network: Network; disabled: boolean; nodeConfigured: boolean;
  onOpened: () => void; onConfigure: () => void
}) {
  const client = useQueryClient()
  const wallets = useQuery({ queryKey: ["wallet-list", root, network], queryFn: listWallets })
  const runtime = useQuery({ queryKey: ["wallet-runtime-ready"], queryFn: walletRuntimeReady })
  const [walletId, setWalletId] = useState("")
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const opening = useRef(false)
  const passwordInput = useRef<HTMLInputElement>(null)
  const singleWallet = wallets.data?.length === 1 ? wallets.data[0] : undefined
  const selected = wallets.data?.find((wallet) => wallet.id === walletId) ?? singleWallet

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (opening.current || disabled || !nodeConfigured || runtime.data !== true || !selected) return
    const password = passwordInput.current?.value ?? ""
    if (!password) return
    passwordInput.current!.value = ""
    opening.current = true
    setBusy(true)
    setError(null)
    try {
      const status = await openWallet(selected.id, password)
      client.setQueryData(["foundation-status"], status)
      client.setQueryData(["active-wallet", status.session_generation], selected)
      void client.invalidateQueries({ queryKey: ["active-wallet", status.session_generation] })
      onOpened()
    } catch (cause) {
      setError(String(cause))
      void client.invalidateQueries({ queryKey: ["foundation-status"] })
    } finally {
      opening.current = false
      setBusy(false)
    }
  }

  if (wallets.isSuccess && wallets.data.length === 0) return null

  return <section aria-label="Your saved wallets" className="rounded-xl border border-sky-500/30 bg-[var(--app-surface)] p-4">
    <div className="flex items-center gap-3">
      <span aria-hidden="true" className="flex size-9 shrink-0 items-center justify-center rounded-lg bg-sky-400/10 text-sky-200">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" className="size-5">
          <rect x="5" y="10" width="14" height="11" rx="2" /><path d="M8 10V7a4 4 0 0 1 8 0v3M12 14v3" />
        </svg>
      </span>
      <div className="min-w-0 flex-1"><h2 className="font-semibold">Unlock a wallet</h2>
        <p className="mt-0.5 text-xs text-slate-400">{singleWallet ? "Enter your password to continue." : "Choose a saved wallet to continue."}</p>
      </div>
      {wallets.isSuccess ? <span className="shrink-0 text-xs text-slate-400">{wallets.data.length} saved</span> : null}
    </div>
    {wallets.isPending ? <p role="status" className="mt-3 text-sm text-slate-400">Loading your wallets…</p>
      : wallets.isError ? <p role="alert" className="mt-3 text-sm text-red-300">Saved wallets could not be read. <button type="button" className="underline" onClick={() => void wallets.refetch()}>Retry</button></p>
        : <form onSubmit={(event) => void submit(event)} className="mt-4 grid gap-3">
          <fieldset disabled={disabled || busy} className="contents">
            {singleWallet ? <div className="min-w-0 rounded-lg bg-[var(--app-input)] px-3 py-2">
              <p className="truncate text-sm font-semibold" title={singleWallet.name || `Wallet ${singleWallet.id.slice(0, 8)}`}>{singleWallet.name || `Wallet ${singleWallet.id.slice(0, 8)}`}</p>
              {singleWallet.name ? <p className="mt-0.5 font-mono text-xs text-slate-400">ID · {singleWallet.id.slice(0, 8)}</p> : null}
            </div> : <label className="grid gap-1 text-sm"><span>Wallet</span>
              <select required value={selected?.id ?? ""} onChange={(event) => { setWalletId(event.target.value); setError(null) }}
                className="w-full min-w-0 rounded-md border border-slate-600 bg-[var(--app-input)] px-3 py-2">
                <option value="">Choose a wallet</option>
                {wallets.data?.map((wallet) => <option key={wallet.id} value={wallet.id}>{wallet.name ? `${wallet.name} · ${wallet.id.slice(0, 8)}` : `Wallet ${wallet.id.slice(0, 8)}`}{wallet.backup_complete ? "" : " · backup pending"}</option>)}
              </select>
            </label>}
            {selected && !selected.backup_complete ? <p className="text-xs text-amber-200">Backup pending. Unlock to finish your recovery backup.</p> : null}
            {selected ? <div className="grid items-end gap-2 sm:grid-cols-[minmax(0,1fr)_auto]">
              <label className="grid min-w-0 gap-1 text-sm"><span>Wallet password</span>
                <input key={selected.id} ref={passwordInput} name="password" type="password" required minLength={1} autoComplete="current-password"
                  className="w-full min-w-0 rounded-md border border-slate-600 bg-[var(--app-input)] px-3 py-2" />
              </label>
              <Button type="submit" size="sm" disabled={!nodeConfigured || runtime.data !== true} className="h-[38px] w-full bg-sky-400 text-slate-950 hover:bg-sky-300 sm:w-auto">
                {busy ? "Opening…" : "Unlock wallet"}
              </Button>
            </div> : null}
          </fieldset>
          {error ? <p role="alert" className="text-sm text-red-300">{error}</p> : null}
          {runtime.isError || runtime.data === false ? <p role="alert" className="text-xs text-amber-200">The wallet runtime is unavailable. <button type="button" className="underline" onClick={() => void runtime.refetch()}>Check again</button></p> : null}
          {!nodeConfigured && !disabled ? <p className="text-xs text-slate-400">Select a node once before opening a wallet. <button type="button" className="text-sky-300 underline" onClick={onConfigure}>Choose a node</button></p> : null}
        </form>}
  </section>
}
