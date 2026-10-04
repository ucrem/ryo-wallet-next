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
  const selected = wallets.data?.find((wallet) => wallet.id === walletId)

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

  return <section aria-label="Your saved wallets" className="rounded-xl border border-slate-700 bg-[var(--app-surface)] p-4">
    <h2 className="font-semibold">Your wallets</h2>
    <p className="mt-1 text-xs text-slate-400">Reopen a saved wallet with its password. No setup or import is needed.</p>
    {wallets.isPending ? <p role="status" className="mt-3 text-sm text-slate-400">Loading your wallets…</p>
      : wallets.isError ? <p role="alert" className="mt-3 text-sm text-red-300">Saved wallets could not be read. <button type="button" className="underline" onClick={() => void wallets.refetch()}>Retry</button></p>
        : <form onSubmit={(event) => void submit(event)} className="mt-3 grid gap-3 sm:grid-cols-2">
          <fieldset disabled={disabled || busy} className="contents">
            <label className="grid gap-1 text-sm"><span>Saved wallet</span>
              <select required value={selected?.id ?? ""} onChange={(event) => { setWalletId(event.target.value); setError(null) }}
                className="w-full min-w-0 rounded-md border border-slate-600 bg-[var(--app-input)] px-3 py-1.5">
                <option value="">Choose a wallet</option>
                {wallets.data?.map((wallet) => <option key={wallet.id} value={wallet.id}>Wallet {wallet.id.slice(0, 8)}{wallet.backup_complete ? "" : " · backup pending"}</option>)}
              </select>
            </label>
            {selected ? <label className="grid gap-1 text-sm"><span>Wallet password</span>
              <input key={selected.id} ref={passwordInput} name="password" type="password" required minLength={1} autoComplete="current-password"
                className="w-full min-w-0 rounded-md border border-slate-600 bg-[var(--app-input)] px-3 py-1.5" />
            </label> : null}
            <Button type="submit" size="sm" disabled={!selected || !nodeConfigured || runtime.data !== true} className="justify-self-start bg-sky-400 text-slate-950 hover:bg-sky-300 sm:col-span-2">
              {busy ? "Opening…" : "Unlock wallet"}
            </Button>
          </fieldset>
          {error ? <p role="alert" className="text-sm text-red-300 sm:col-span-2">{error}</p> : null}
          {runtime.isError || runtime.data === false ? <p role="alert" className="text-xs text-amber-200 sm:col-span-2">The wallet runtime is unavailable. <button type="button" className="underline" onClick={() => void runtime.refetch()}>Check again</button></p> : null}
          {!nodeConfigured && !disabled ? <p className="text-xs text-slate-400 sm:col-span-2">Select a node once before opening a wallet. <button type="button" className="text-sky-300 underline" onClick={onConfigure}>Choose a node</button></p> : null}
        </form>}
  </section>
}
