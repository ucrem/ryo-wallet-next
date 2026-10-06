import { useEffect, useRef, useState, type FormEvent, type ReactNode } from "react"
import { useQuery, useQueryClient } from "@tanstack/react-query"
import { writeText } from "@tauri-apps/plugin-clipboard-manager"
import { openUrl } from "@tauri-apps/plugin-opener"
import { Button } from "@/components/ui/button"
import { getWalletOverview } from "@/api/overview"
import { createReceiveAddress, getReceiveAddresses, getWalletSyncStatus } from "@/api/wallet"
import { exportArtwork, manageKeyImages, removeWallet, walletOperation, type AddressBalance, type Contact, type SecretMaterial, type SendDraft, type SendEntry, type Transaction, type WalletSection } from "@/api/operations"
import { exactAmount, formatRyo, transactionDate } from "@/lib/walletFormatting"
import { identiconArtwork, qrArtwork, WalletArtwork } from "./WalletArtwork"
import { usePreferences } from "@/lib/usePreferences"

const inputClass = "w-full rounded-md border border-slate-600 bg-[var(--app-input)] px-3 py-1.5 text-sm focus-visible:outline-sky-400 disabled:opacity-50"
const cardClass = "rounded-xl border border-slate-700 bg-[var(--app-surface)] p-5"
const compactCardClass = "rounded-xl border border-slate-700 bg-[var(--app-surface)] p-4"
type SessionProps = { generation: string }

function Field({ label, children }: { label: string; children: ReactNode }) {
  return <label className="grid content-start gap-1.5 text-sm"><span className="text-slate-300">{label}</span>{children}</label>
}
function Alert({ message }: { message: string | null }) {
  return message ? <p role="alert" className="mt-4 rounded-md border border-red-500/30 bg-red-500/10 p-3 text-sm text-red-200">{message}</p> : null
}
function CopyButton({ value, label = "Copy" }: { value: string; label?: string }) {
  return <CopyValueButton key={value} value={value} label={label} />
}
function CopyValueButton({ value, label }: { value: string; label: string }) {
  const [status, setStatus] = useState("")
  return <Button type="button" variant="outline" size="sm" disabled={!value} onClick={() => {
    void writeText(value).then(() => setStatus("Copied"), () => setStatus("Copy failed"))
  }}>{status || label}</Button>
}
function useWalletData<T>(generation: string, type: "contacts" | "history" | "info" | "address_balances" | "send_journal", refresh = false) {
  return useQuery({ queryKey: ["wallet-operation", generation, type], queryFn: () => walletOperation<T>(generation, { type }), retry: false,
    refetchInterval: (query) => refresh || (type !== "send_journal" && query.state.status === "error") ? 15_000 : false,
    refetchIntervalInBackground: false })
}

function walletRpcBusy(error: unknown) { return String(error) === "wallet RPC busy" }

function DataRefreshIndicator({ subject, hasSnapshot, refreshing }: { subject: "balances" | "history"; hasSnapshot: boolean; refreshing: boolean }) {
  const detail = `${refreshing ? "Refreshing wallet data." : "Waiting for updated wallet data."} ${hasSnapshot ? `Showing the last available ${subject}. ` : ""}${subject === "history" ? "Transactions" : "Balances"} update automatically.`
  return <span role="status" title={detail} aria-label={detail}
    className="inline-flex shrink-0 items-center gap-1.5 rounded-full border border-amber-500/20 bg-amber-500/5 px-2 py-0.5 text-xs font-normal text-amber-200">
    <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round"
      className={refreshing ? "size-3 animate-spin [animation-duration:3s] motion-reduce:animate-none" : "size-3"}>
      {refreshing ? <path d="M20 7v5h-5M4 17v-5h5M6.1 6.1A8 8 0 0 1 20 12M17.9 17.9A8 8 0 0 1 4 12" />
        : <><circle cx="12" cy="12" r="8" /><path d="M12 8v4l2 2" /></>}
    </svg>
    {refreshing ? "Updating" : "Update pending"}
  </span>
}

type DashboardProps = SessionProps & {
  section: WalletSection; onSection: (section: WalletSection) => void; onLock: () => void; onRemoved: () => void; locking: boolean
}

export function WalletDashboard(props: DashboardProps) {
  const [hidden, setHidden] = useState(false)
  return <WalletDashboardPage key={`${props.generation}:${props.section}`} {...props}
    hidden={hidden} onToggleBalances={() => setHidden((value) => !value)} />
}

function WalletDashboardPage({ generation, section, onSection, onLock, onRemoved, locking, hidden, onToggleBalances }: DashboardProps & {
  hidden: boolean; onToggleBalances: () => void
}) {
  const queryClient = useQueryClient()
  const overview = useQuery({ queryKey: ["wallet-overview", generation], queryFn: getWalletOverview, refetchInterval: 10_000 })
  const info = useWalletData<{ name: string }>(generation, "info")
  useEffect(() => {
    if (!info.isSuccess) return
    // A delayed authenticated name read can fill an older wallet's display
    // cache after unlock. Refresh both pickers before the next lock.
    void queryClient.invalidateQueries({ queryKey: ["wallet-list"] })
    void queryClient.invalidateQueries({ queryKey: ["active-wallet", generation] })
  }, [queryClient, generation, info.isSuccess, info.data?.name])
  const history = useWalletData<Transaction[]>(generation, "history", true)
  const [action, setAction] = useState<string | null>(null)
  const address = overview.data?.primary_address ?? ""
  const compact = section !== "overview"
  const balances = (["total", "unlocked", "locked"] as const).map((key) => <div key={key}>
    <p className="text-xs uppercase tracking-wide text-slate-400">{key}</p>
    <p className={compact ? "font-mono text-sm" : "mt-1 font-mono text-lg"}>{hidden ? "••••" : overview.data ? `${formatRyo(overview.data[key].atomic)} RYO` : "—"}</p>
  </div>)
  return <div className="grid gap-4">
    <section aria-label="Wallet summary" className={compact ? "rounded-xl border border-slate-700 bg-[var(--app-surface)] px-4 py-3" : cardClass}>
      <div className={`flex justify-between gap-5 ${compact ? "items-center" : "items-start"}`}>
        <div className={`flex min-w-0 gap-3 ${compact ? "items-center" : "items-start"}`}>
          {address ? <WalletArtwork artwork={identiconArtwork(address)} label="Wallet address identicon" className={`${compact ? "size-9" : "size-14"} shrink-0 rounded-md`} /> : null}
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <h1 className={`${compact ? "max-w-64 truncate text-lg" : "text-xl"} font-semibold`}>{info.data?.name || "My wallet"}</h1>
              {overview.isError && walletRpcBusy(overview.error) ? <DataRefreshIndicator subject="balances" hasSnapshot={overview.data !== undefined} refreshing={overview.isFetching} /> : null}
            </div>
            {!compact ? <p className="mt-1 break-all font-mono text-xs text-slate-300">{address || "Loading address…"}</p> : null}
          </div>
        </div>
        {compact ? <div className="flex shrink-0 gap-6">{balances}</div> : null}
        <div className="flex shrink-0 gap-2">
          {!compact ? <CopyButton value={address} label="Copy address" /> : null}
          <Button variant="outline" size="sm" onClick={onToggleBalances}>{hidden ? "Show balances" : "Hide balances"}</Button>
          <details className="relative">
            <summary className="cursor-pointer rounded-md border border-slate-600 px-3 py-1.5 text-sm hover:bg-slate-800">Wallet actions</summary>
            <div className="absolute right-0 top-full z-20 mt-2 grid w-56 rounded-lg border border-slate-600 bg-slate-900 p-1 shadow-xl">
              {[["name", "Rename wallet"], ["secrets", "Show private keys"], ["password", "Change password"], ["rescan", "Rescan wallet"], ["images", "Manage key images"], ["remove", "Delete wallet"]].map(([key, label]) =>
                <button key={key} type="button" className="rounded-md px-3 py-2 text-left text-sm hover:bg-slate-800" onClick={(event) => {
                  event.currentTarget.closest("details")?.removeAttribute("open"); setAction(key)
                }}>{label}</button>)}
            </div>
          </details>
          <Button variant="outline" size="sm" disabled={locking} onClick={onLock}>{locking ? "Locking…" : "Lock"}</Button>
        </div>
      </div>
      {!compact ? <div className="mt-4 grid grid-cols-3 gap-4 border-t border-slate-700 pt-3">{balances}</div> : null}
      <Alert message={overview.isError && !walletRpcBusy(overview.error) ? `Balances could not be updated. Retrying automatically.${overview.data ? " Showing the last available balances." : ""}` : null} />
    </section>
    {action ? <WalletActions key={action} generation={generation} action={action} name={info.data?.name ?? ""} onClose={() => setAction(null)} onRemoved={onRemoved} /> : null}
    {section === "overview" ? <section className={cardClass}>
      <div className="flex items-center justify-between gap-3"><h2 className="text-lg font-semibold">Recent transactions</h2>
        <Button variant="ghost" size="sm" onClick={() => onSection("history")}>View all →</Button></div>
      <Transactions generation={generation} entries={history.data?.slice(0, 5)} loading={history.isPending} failed={history.isError} busy={walletRpcBusy(history.error)} hidden={hidden} />
    </section> : null}
    {section === "receive" ? <ReceivePanel generation={generation} /> : null}
    {section === "send" ? <SendPanel generation={generation} unlocked={overview.data?.unlocked.atomic ?? "0"} onHistory={() => onSection("history")} /> : null}
    {section === "contacts" ? <ContactsPanel generation={generation} onSend={() => onSection("send")} /> : null}
    {section === "history" ? <HistoryPanel generation={generation} entries={history.data} loading={history.isPending} failed={history.isError} busy={walletRpcBusy(history.error)} refreshing={history.isFetching} /> : null}
  </div>
}

function ReceivePanel({ generation }: SessionProps) {
  const client = useQueryClient()
  const addresses = useQuery({ queryKey: ["receive-addresses", generation], queryFn: () => getReceiveAddresses(generation), retry: false,
    refetchInterval: (query) => query.state.status === "error" ? 15_000 : false })
  const balances = useWalletData<AddressBalance[]>(generation, "address_balances", true)
  const [index, setIndex] = useState(0)
  const [uri, setUri] = useState("")
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const entry = addresses.data?.find((entry) => entry.address_index === index)
  const balance = balances.data?.find((entry) => entry.index === index)
  const qr = entry ? qrArtwork(uri || entry.address) : null
  async function run(action: () => Promise<unknown>) {
    if (busy) return; setBusy(true); setError(null)
    try { await action() } catch (cause) { setError(String(cause)) } finally { setBusy(false) }
  }
  return <div className="grid items-start gap-4 lg:grid-cols-[300px_minmax(0,1fr)]">
    <section className={compactCardClass}>
      <div className="flex items-center justify-between gap-2"><h2 className="text-lg font-semibold">Addresses</h2>
        <Button size="sm" disabled={busy || !addresses.data} onClick={() => void run(async () => {
          try { const created = await createReceiveAddress(generation); setIndex(created.address_index); setUri("") }
          finally { await client.invalidateQueries({ queryKey: ["receive-addresses"] }) }
        })}>New address</Button></div>
      <div aria-label="Receive address selection" className="mt-3 grid max-h-80 gap-2 overflow-y-auto">
        {addresses.data?.map((address) => <button type="button" key={address.address_index} aria-pressed={index === address.address_index}
          onClick={() => { setIndex(address.address_index); setUri("") }} className={`min-w-0 rounded-md border px-3 py-2 text-left ${index === address.address_index ? "border-sky-400 bg-sky-400/5" : "border-slate-700 hover:bg-slate-800"}`}>
          <strong className="block truncate text-sm">{address.label || (address.address_index === 0 ? "Primary address" : `Address #${address.address_index}`)}</strong>
          <span className="mt-1 block text-xs text-slate-400">#{address.address_index} · {address.used ? "Used" : "Unused"}</span>
        </button>)}
      </div>
      <p className="mt-3 text-xs leading-relaxed text-slate-400">Select an address to receive on. All share one recovery phrase; recreate them in the same order after seed recovery.</p>
      <Alert message={addresses.isError ? "Receive addresses could not be loaded." : error} />
    </section>
    <section className={compactCardClass}>
      {entry && qr ? <>
        <div className="grid gap-4 md:grid-cols-[160px_minmax(0,1fr)]">
          <WalletArtwork artwork={qr} label="Receive QR code" className="size-40" />
          <div className="min-w-0">
            <h2 className="text-lg font-semibold">{uri ? "Payment request" : "Address details"}</h2>
            <p className="mt-1 break-all font-mono text-xs leading-relaxed text-slate-300">{uri || entry.address}</p>
            <div className="mt-2 flex gap-5 text-sm"><p>Balance: {balance ? `${formatRyo(balance.balance)} RYO` : "—"}</p><p>Unlocked: {balance ? `${formatRyo(balance.unlocked)} RYO` : "—"}</p></div>
            <div className="mt-2 flex flex-wrap gap-2">
              <CopyButton value={uri || entry.address} label={uri ? "Copy request" : "Copy address"} />
              <Button variant="outline" size="sm" disabled={busy} onClick={() => void run(() => exportArtwork(qr))}>Save QR code</Button>
              <Button variant="outline" size="sm" disabled={busy} onClick={() => void run(() => exportArtwork(identiconArtwork(entry.address)))}>Save identicon</Button>
            </div>
            <form key={index} className="mt-3 flex items-end gap-3" onSubmit={(event) => {
              event.preventDefault(); const label = String(new FormData(event.currentTarget).get("label") ?? "")
              void run(async () => { await walletOperation(generation, { type: "label_address", index, label }); await client.invalidateQueries({ queryKey: ["receive-addresses"] }) })
            }}><div className="min-w-0 flex-1"><Field label="Address label"><input name="label" defaultValue={entry.label} maxLength={100} className={inputClass} /></Field></div><Button type="submit" size="sm" disabled={busy}>Save label</Button></form>
          </div>
        </div>
        <form key={`request:${index}`} className="mt-4 border-t border-slate-700 pt-3" onSubmit={(event) => {
          event.preventDefault(); const values = new FormData(event.currentTarget)
          void run(async () => { const result = await walletOperation<{ uri: string }>(generation, { type: "make_request", address: entry.address,
            amount: String(values.get("amount") ?? ""), payment_id: String(values.get("payment_id") ?? ""), description: String(values.get("description") ?? "") }); setUri(result.uri) })
        }}>
          <h3 className="font-medium">Request a payment</h3>
          <div className="mt-2 grid items-end gap-3 md:grid-cols-[1fr_1fr_1.4fr_auto]">
            <Field label="Amount RYO (optional)"><input name="amount" inputMode="decimal" className={inputClass} placeholder="0" /></Field>
            <Field label="Payment ID (optional)"><input name="payment_id" maxLength={64} className={inputClass} /></Field>
            <Field label="Description (optional)"><input name="description" maxLength={500} className={inputClass} /></Field>
            <Button type="submit" size="sm" disabled={busy}>Create request</Button>
          </div>
          {uri ? <Button variant="ghost" size="sm" className="mt-2" onClick={() => setUri("")}>Show address QR</Button> : null}
        </form>
      </> : <p className="mt-4 text-sm text-slate-400">Loading address…</p>}
    </section>
  </div>
}

function ContactsPanel({ generation, onSend }: SessionProps & { onSend: () => void }) {
  const client = useQueryClient()
  const contacts = useWalletData<Contact[]>(generation, "contacts")
  const [editing, setEditing] = useState<Contact | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [filter, setFilter] = useState("")
  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); if (busy) return
    const form = event.currentTarget; const data = new FormData(form); setBusy(true); setError(null)
    try {
      await walletOperation(generation, { type: "save_contact", id: editing?.id ?? null, name: String(data.get("name") ?? ""),
        address: String(data.get("address") ?? "").trim(), payment_id: String(data.get("payment_id") ?? "").trim(), notes: String(data.get("notes") ?? "") })
      setEditing(null); form.reset()
    } catch (cause) { setError(String(cause)) } finally { await contacts.refetch(); setBusy(false) }
  }
  return <div className="grid items-start gap-4 lg:grid-cols-2">
    <section className={compactCardClass}><h2 className="text-lg font-semibold">Address book</h2>
      <input aria-label="Search contacts" placeholder="Search name or address" value={filter} onChange={(e) => setFilter(e.target.value)} className={`${inputClass} mt-3`} />
      <div className="mt-3 grid max-h-[420px] gap-2 overflow-y-auto">{contacts.data?.filter((c) => `${c.name} ${c.address}`.toLowerCase().includes(filter.toLowerCase())).map((contact) =>
        <div key={contact.id} className="rounded-lg border border-slate-700 p-3">
          <div className="flex items-center justify-between gap-3"><h3 className="min-w-0 truncate font-medium">{contact.name}</h3>
          <div className="flex shrink-0 gap-1"><Button size="sm" disabled={busy} onClick={() => { client.setQueryData(["send-contact", generation], contact); onSend() }}>Send</Button>
            <CopyButton value={contact.address} label="Copy address" /><Button size="sm" variant="outline" onClick={() => { setEditing(contact); setError(null) }}>Edit</Button>
            <Button size="sm" variant="ghost" disabled={busy} onClick={() => { if (!window.confirm(`Delete contact ${contact.name}?`)) return
              setBusy(true); setError(null); void walletOperation(generation, { type: "delete_contact", id: contact.id }).then(() => {
                if (client.getQueryData<Contact>(["send-contact", generation])?.id === contact.id) {
                  client.removeQueries({ queryKey: ["send-contact", generation], exact: true })
                }
                setEditing((current) => current?.id === contact.id ? null : current)
              }).catch((cause: unknown) => setError(String(cause)))
                .finally(async () => { await contacts.refetch(); setBusy(false) }) }}>Delete</Button></div></div>
          <p className="mt-1 break-all font-mono text-xs text-slate-300">{contact.address}</p>
          {contact.payment_id ? <p className="mt-1 break-all text-xs text-slate-400">Payment ID: {contact.payment_id}</p> : null}
          {contact.notes ? <p className="mt-1 line-clamp-2 whitespace-pre-wrap text-sm text-slate-400">{contact.notes}</p> : null}
        </div>)}
        {contacts.data?.length === 0 ? <p className="py-4 text-sm text-slate-400">No saved contacts yet.</p> : null}
      </div><Alert message={contacts.isError ? "Contacts could not be loaded." : null} />
    </section>
    <section className={compactCardClass}><h2 className="text-lg font-semibold">{editing ? "Edit contact" : "New contact"}</h2>
      <form key={editing?.id ?? "new"} onSubmit={(event) => void save(event)} className="mt-3 grid gap-3 md:grid-cols-2">
        <Field label="Name"><input name="name" required maxLength={100} defaultValue={editing?.name} className={inputClass} /></Field>
        <Field label="Payment ID (optional)"><input name="payment_id" maxLength={64} defaultValue={editing?.payment_id} className={inputClass} /></Field>
        <div className="md:col-span-2"><Field label="Ryo address"><textarea name="address" required rows={2} maxLength={256} defaultValue={editing?.address} className={`${inputClass} resize-none font-mono`} /></Field></div>
        <div className="md:col-span-2"><Field label="Notes"><textarea name="notes" rows={2} maxLength={2000} defaultValue={editing?.notes} className={`${inputClass} resize-none`} /></Field></div>
        <div className="flex gap-3 md:col-span-2"><Button type="submit" size="sm" disabled={busy}>{busy ? "Saving…" : "Save contact"}</Button>
          {editing ? <Button variant="outline" size="sm" onClick={() => setEditing(null)}>Cancel</Button> : null}</div>
      </form><Alert message={error} />
    </section>
  </div>
}

function SendPanel({ generation, unlocked, onHistory }: SessionProps & { unlocked: string; onHistory: () => void }) {
  const preferences = usePreferences()
  const client = useQueryClient()
  const contacts = useWalletData<Contact[]>(generation, "contacts")
  const sync = useQuery({ queryKey: ["wallet-send-sync", generation], queryFn: getWalletSyncStatus, refetchInterval: 5000 })
  const contactIntent = client.getQueryData<Contact>(["send-contact", generation])
  const selected = contacts.data?.find((contact) => contact.id === contactIntent?.id)
  const [address, setAddress] = useState(selected?.address ?? "")
  const [paymentId, setPaymentId] = useState(selected?.payment_id ?? "")
  const [contactId, setContactId] = useState(selected?.id ?? "")
  const [amount, setAmount] = useState("")
  const [sweep, setSweep] = useState(false)
  const [draft, setDraft] = useState<SendDraft | null>(null)
  const draftRef = useRef<SendDraft | null>(null)
  const [result, setResult] = useState<SendEntry[] | null>(null)
  const [busy, setBusy] = useState(false)
  const guard = useRef(false)
  const [error, setError] = useState<string | null>(null)
  const [saveContact, setSaveContact] = useState<{ name: string; notes: string } | null>(null)
  const ready = sync.data?.node_reachable && sync.data.node_ready && !sync.data.node_offline && sync.data.wallet_height !== null && sync.data.daemon_height !== null && BigInt(sync.data.wallet_height) >= BigInt(sync.data.daemon_height)
  useEffect(() => {
    // A contact selected in Address Book is a one-use navigation intent.
    client.removeQueries({ queryKey: ["send-contact", generation], exact: true })
  }, [client, generation])
  useEffect(() => () => { if (draftRef.current) void walletOperation(generation, { type: "cancel_send", token: draftRef.current.token }).catch(() => {}) }, [generation])
  async function prepare(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); if (guard.current) return
    if (!address || (!sweep && !exactAmount(amount))) { setError("Enter an address and an exact positive amount with at most nine decimal places."); return }
    const values = new FormData(event.currentTarget)
    guard.current = true; setBusy(true); setError(null)
    try {
      const preview = await walletOperation<SendDraft>(generation, { type: "prepare_send", address: address.trim(), amount, sweep, payment_id: paymentId.trim(),
        priority: Number(values.get("priority")), ring_size: Number(values.get("ring_size")) })
      const contactName = String(values.get("contact_name") ?? "").trim()
      setSaveContact(contactName ? { name: contactName, notes: String(values.get("contact_notes") ?? "") } : null)
      draftRef.current = preview; setDraft(preview)
    } catch (cause) { setError(String(cause)) } finally { guard.current = false; setBusy(false) }
  }
  async function confirm() {
    if (!draft || guard.current) return
    guard.current = true; setBusy(true); setError(null)
    const preview = draft; draftRef.current = null
    try {
      const output = await walletOperation<{ transactions: SendEntry[]; wallet_saved: boolean }>(generation, { type: "confirm_send", token: preview.token })
      setResult(output.transactions)
      if (!output.wallet_saved) setError("Send outcomes are recorded, but saving the wallet failed. Check history before sending again.")
      if (saveContact && output.transactions.some((entry) => entry.state === "relayed")) {
        try { await walletOperation(generation, { type: "save_contact", id: null, name: saveContact.name, notes: saveContact.notes, address: preview.address, payment_id: preview.payment_id }) }
        catch { setError("The send result is recorded, but the contact could not be saved.") }
      }
    } catch (cause) { setError(`${String(cause)}. Check transaction history before preparing another send.`) }
    finally {
      setDraft(null); guard.current = false; setBusy(false)
      await client.invalidateQueries({ queryKey: ["wallet-operation", generation] }); await client.invalidateQueries({ queryKey: ["wallet-overview"] })
    }
  }
  return <section className={compactCardClass}>
    <div className="flex items-center justify-between gap-4"><h2 className="text-lg font-semibold">Send Ryo</h2><p className="text-sm text-slate-400">Unlocked: {formatRyo(unlocked)} RYO · Review the exact fee before confirming.</p></div>
    {result ? <><SendResults entries={result} /><div className="mt-4 flex gap-3"><Button onClick={onHistory}>View transaction history</Button><Button variant="outline" disabled={result.some((entry) => entry.state === "unknown")} onClick={() => { setResult(null); setAddress(""); setAmount(""); setPaymentId(""); setContactId(""); setSweep(false) }}>New transaction</Button></div></>
      : draft ? <SendReview draft={draft} busy={busy} warnNoPaymentId={preferences.data?.notify_no_payment_id ?? true} onConfirm={() => void confirm()} onCancel={() => {
        if (guard.current) return; draftRef.current = null; setDraft(null)
        void walletOperation(generation, { type: "cancel_send", token: draft.token }).catch((cause: unknown) => setError(String(cause)))
      }} /> : <>
        {!ready ? <p role="status" className="mt-2 text-sm text-amber-200">Both node and wallet must finish syncing before a send can be prepared.</p> : null}
        <form onSubmit={(event) => void prepare(event)} className="mt-3 grid gap-3 md:grid-cols-4">
          <div className="md:col-span-3"><Field label="Recipient address"><input required value={address} maxLength={256} onChange={(e) => { setAddress(e.target.value); setContactId("") }} className={`${inputClass} font-mono`} disabled={busy} /></Field></div>
          <Field label="Choose from address book"><select className={inputClass} value={contacts.data?.some((c) => c.id === contactId && c.address === address && c.payment_id === paymentId) ? contactId : ""} disabled={busy} onChange={(e) => {
            setContactId(e.target.value)
            const contact = contacts.data?.find((c) => c.id === e.target.value); if (contact) { setAddress(contact.address); setPaymentId(contact.payment_id) }
          }}><option value="">Select a contact…</option>{contacts.data?.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}</select></Field>
          <div><Field label="Amount (RYO)"><input inputMode="decimal" value={sweep ? "" : amount} disabled={busy || sweep} placeholder={sweep ? "All unlocked coins, minus the fee" : "0.000000000"} onChange={(e) => setAmount(e.target.value)} className={inputClass} /></Field>
          <label className="mt-1.5 flex items-center gap-2 text-xs"><input type="checkbox" checked={sweep} disabled={busy} onChange={(e) => setSweep(e.target.checked)} />Send all unlocked coins</label></div>
          <Field label="Payment ID (optional)"><input value={paymentId} maxLength={64} onChange={(e) => { setPaymentId(e.target.value); setContactId("") }} className={inputClass} disabled={busy} /></Field>
          <Field label="Priority"><select name="priority" defaultValue="0" className={inputClass} disabled={busy}>{["Normal", "High ×2", "High ×4", "High ×20", "Highest ×144"].map((label, index) => <option key={index} value={index}>{label}</option>)}</select></Field>
          <Field label="Ring size"><select name="ring_size" defaultValue="25" className={inputClass} disabled={busy}><option value="25">25 members (default)</option><option value="100">100 members</option></select></Field>
          <div className="md:col-span-2"><Field label="Save recipient as contact (optional name)"><input name="contact_name" maxLength={100} className={inputClass} disabled={busy} /></Field></div>
          <div className="md:col-span-2"><Field label="Contact notes (optional)"><input name="contact_notes" maxLength={2000} className={inputClass} disabled={busy} /></Field></div>
          <Button type="submit" size="sm" className="justify-self-start md:col-span-4" disabled={busy || !ready || BigInt(unlocked) === 0n}>{busy ? "Preparing…" : "Review transaction →"}</Button>
        </form>
        <form className="mt-3 flex items-end gap-3 border-t border-slate-700 pt-3" onSubmit={(event) => {
          event.preventDefault(); if (guard.current) return
          const uri = String(new FormData(event.currentTarget).get("uri") ?? ""); guard.current = true; setBusy(true); setError(null)
          void walletOperation<{ address: string; amount: string; payment_id: string }>(generation, { type: "parse_request", uri })
            .then((request) => { setAddress(request.address); setAmount(request.amount); setPaymentId(request.payment_id); setContactId(""); setSweep(false) })
            .catch((cause: unknown) => setError(String(cause))).finally(() => { guard.current = false; setBusy(false) })
        }}><div className="min-w-0 flex-1"><Field label="Load a Ryo payment request"><input name="uri" placeholder="ryo:…" className={inputClass} maxLength={4096} /></Field></div><Button type="submit" size="sm" variant="outline" disabled={busy}>Load request</Button></form>
      </>}
    <Alert message={error} />
  </section>
}

export function SendReview({ draft, busy, onConfirm, onCancel, warnNoPaymentId = false }: { draft: SendDraft; busy: boolean; onConfirm: () => void; onCancel: () => void; warnNoPaymentId?: boolean }) {
  const [confirmed, setConfirmed] = useState(false)
  const [paymentIdChecked, setPaymentIdChecked] = useState(false)
  const missingIdWarning = warnNoPaymentId && !draft.payment_id
  const [expiry] = useState(() => Date.now() + draft.expires_in_seconds * 1000)
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => { const timer = window.setInterval(() => setNow(Date.now()), 1000); return () => window.clearInterval(timer) }, [])
  const expired = now >= expiry
  return <div className="mt-4 rounded-lg border border-sky-500/40 bg-sky-500/5 p-5">
    <h3 className="font-semibold">Confirm this transaction</h3>
    <p className="mt-3 text-xs uppercase text-slate-400">Recipient</p><p className="mt-1 break-all font-mono text-sm">{draft.address}</p>
    {draft.payment_id ? <p className="mt-2 break-all text-sm">Payment ID: {draft.payment_id}</p> : null}
    {missingIdWarning ? <div role="alert" className="mt-3 rounded-md border border-amber-500/40 bg-amber-500/5 p-3 text-sm text-amber-200">
      <p>No Payment ID is included. If the recipient requires one, the payment may not be credited.</p>
      <label className="mt-2 flex items-center gap-2"><input type="checkbox" checked={paymentIdChecked} onChange={(event) => setPaymentIdChecked(event.target.checked)} disabled={busy || expired} />I checked that the recipient does not require a separate Payment ID.</label>
    </div> : null}
    <dl className="mt-5 grid grid-cols-3 gap-4">{[["Amount", draft.amount], ["Network fee", draft.fee], ["Total debit", draft.total]].map(([label, amount]) => <div key={label}><dt className="text-sm text-slate-400">{label}</dt><dd className="mt-1 font-mono text-lg">{formatRyo(amount)} RYO</dd></div>)}</dl>
    <p className="mt-4 text-xs text-slate-400">{draft.transactions.length} transaction(s). {expired ? "Preview expired: cancel and prepare again." : `Preview expires in ${Math.max(0, Math.ceil((expiry - now) / 1000))} seconds.`}</p>
    <label className="mt-4 flex items-center gap-2 text-sm"><input type="checkbox" checked={confirmed} onChange={(e) => setConfirmed(e.target.checked)} disabled={busy || expired} />I checked the full address, amount and fee.</label>
    <div className="mt-4 flex gap-3"><Button disabled={busy || expired || !confirmed || (missingIdWarning && !paymentIdChecked)} onClick={onConfirm}>{busy ? "Submitting…" : "Confirm and send"}</Button><Button variant="outline" disabled={busy} onClick={onCancel}>Cancel</Button></div>
  </div>
}
function SendResults({ entries }: { entries: SendEntry[] }) {
  return <div className="mt-4 grid gap-3">{entries.map((entry) => <div key={entry.txid} className="rounded-md border border-slate-700 p-3">
    <p className={`text-sm font-semibold ${entry.state === "unknown" ? "text-amber-200" : "text-slate-200"}`}>{entry.state === "unknown" ? "Outcome uncertain — check history before sending again" : entry.state === "not_sent" ? "Not submitted" : entry.state === "relayed" ? "Submitted to the node" : entry.state}</p>
    <p className="mt-1 break-all font-mono text-xs text-slate-400">{entry.txid}</p><div className="mt-2"><CopyButton value={entry.txid} label="Copy transaction ID" /></div>
  </div>)}</div>
}

function HistoryPanel({ generation, entries, loading, failed, busy, refreshing }: SessionProps & { entries: Transaction[] | undefined; loading: boolean; failed: boolean; busy: boolean; refreshing: boolean }) {
  const [type, setType] = useState("all")
  const [search, setSearch] = useState("")
  const [page, setPage] = useState(0)
  const journal = useWalletData<SendEntry[]>(generation, "send_journal")
  const filtered = entries?.filter((entry) => (type === "all" || entry.type === type) && entry.txid.toLowerCase().includes(search.toLowerCase()))
  return <section className={cardClass}><div className="flex items-center justify-between gap-3"><div className="flex flex-wrap items-center gap-2"><h2 className="text-lg font-semibold">Transaction history</h2>
    {failed && busy ? <DataRefreshIndicator subject="history" hasSnapshot={entries !== undefined} refreshing={refreshing} /> : null}</div>
    <Button variant="outline" size="sm" onClick={() => void journal.refetch()} disabled={journal.isFetching}>Check send outcomes</Button></div>
    {journal.data?.some((entry) => entry.state === "unknown" || entry.state === "not_sent") ? <SendResults entries={journal.data.filter((entry) => entry.state === "unknown" || entry.state === "not_sent")} /> : null}
    <div className="mt-4 flex gap-3"><input aria-label="Filter by transaction ID" placeholder="Filter by transaction ID" value={search} onChange={(e) => { setSearch(e.target.value); setPage(0) }} className={`${inputClass} flex-1`} />
      <select aria-label="Transaction type" value={type} onChange={(e) => { setType(e.target.value); setPage(0) }} className={`${inputClass} max-w-48`}>
        {[["all", "All transactions"], ["in", "Incoming"], ["out", "Outgoing"], ["pending", "Pending"], ["pool", "In pool"], ["failed", "Failed"]].map(([value, label]) => <option value={value} key={value}>{label}</option>)}
      </select></div>
    <Transactions generation={generation} entries={filtered?.slice(page * 50, page * 50 + 50)} loading={loading} failed={failed} busy={busy} hidden={false} />
    {(filtered?.length ?? 0) > 50 ? <div className="mt-4 flex items-center gap-3"><Button size="sm" variant="outline" disabled={page === 0} onClick={() => setPage(page - 1)}>Previous</Button><span className="text-sm">Page {page + 1}</span><Button size="sm" variant="outline" disabled={(page + 1) * 50 >= (filtered?.length ?? 0)} onClick={() => setPage(page + 1)}>Next</Button></div> : null}
    <Alert message={journal.isError ? "Send outcomes could not be checked. Do not retry an uncertain send." : null} />
  </section>
}
const transactionTypeLabels: Record<Transaction["type"], string> = {
  in: "Incoming", out: "Outgoing", pool: "Incoming · Unconfirmed", pending: "Outgoing · Pending", failed: "Outgoing · Failed",
}
function Transactions({ generation, entries, loading, failed, busy, hidden }: SessionProps & { entries: Transaction[] | undefined; loading: boolean; failed: boolean; busy: boolean; hidden: boolean }) {
  const [selected, setSelected] = useState<string | null>(null)
  return <div className="mt-3">
    {loading ? <p className="py-4 text-sm text-slate-400">Loading transactions…</p>
      : failed ? busy ? null : <Alert message="Transaction history could not be updated. Retrying automatically." />
        : entries?.length === 0 ? <p className="py-4 text-sm text-slate-400">No transactions found.</p> : null}
    {entries?.map((entry) => {
      const incoming = entry.type === "in" || entry.type === "pool"
      return <div key={`${entry.type}-${entry.txid}-${entry.address}`} className="border-t border-slate-700 first:border-t-0">
      <button type="button" onClick={() => setSelected(selected === entry.txid ? null : entry.txid)} aria-expanded={selected === entry.txid} className="flex w-full items-center justify-between gap-4 py-3 text-left hover:bg-slate-800/40">
        <div className="flex min-w-0 flex-1 items-center gap-3">
          <span aria-hidden="true" title={incoming ? "Incoming transaction" : "Outgoing transaction"} className={`flex size-9 shrink-0 items-center justify-center rounded-full ${incoming ? "bg-emerald-400/10 text-emerald-300" : "bg-red-400/10 text-red-300"}`}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="size-5">
              <path d={incoming ? "M17 7 7 17M7 7v10h10" : "M7 17 17 7M7 7h10v10"} />
            </svg>
          </span>
          <div className="min-w-0"><p className="truncate font-mono text-sm text-sky-200">{entry.txid}</p><p className="mt-1 text-xs text-slate-400">{transactionTypeLabels[entry.type]} · {entry.height !== "0" ? `Height ${entry.height} · ` : ""}{transactionDate(entry.timestamp)}</p></div>
        </div>
        <p className={`shrink-0 font-mono text-sm ${incoming ? "text-emerald-300" : "text-red-300"}`}>{hidden ? "••••" : `${incoming ? "+" : "−"}${formatRyo(entry.amount)} RYO`}</p>
      </button>{selected === entry.txid ? <TransactionDetails key={entry.txid} generation={generation} entry={entry} /> : null}
    </div>})}
  </div>
}
function TransactionDetails({ generation, entry }: SessionProps & { entry: Transaction }) {
  const client = useQueryClient()
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [showRaw, setShowRaw] = useState(false)
  const sync = useQuery({ queryKey: ["wallet-read-sync", generation], queryFn: getWalletSyncStatus, retry: false })
  const confirmations = entry.height !== "0" && sync.data?.daemon_height && BigInt(sync.data.daemon_height) >= BigInt(entry.height)
    ? (BigInt(sync.data.daemon_height) - BigInt(entry.height)).toString() : "0"
  return <div className="mb-4 rounded-md bg-[var(--app-input)] p-4 text-sm">
    <p className="break-all font-mono text-xs">{entry.txid}</p>
    <div className="mt-3 flex gap-3"><CopyButton value={entry.txid} label="Copy transaction ID" /><Button size="sm" variant="outline" onClick={() => void openUrl(`https://explorer.ryo-currency.com/tx/${entry.txid}`).catch(() => setError("Explorer could not be opened."))}>View on explorer</Button><Button size="sm" variant="ghost" onClick={() => setShowRaw(!showRaw)}>{showRaw ? "Hide details" : "Show all details"}</Button></div>
    <dl className="mt-4 grid grid-cols-2 gap-3 text-slate-300"><div><dt>Fee</dt><dd className="font-mono">{formatRyo(entry.fee)} RYO</dd></div><div><dt>Unlock time</dt><dd>{entry.unlock_time}</dd></div>
      <div><dt>Confirmations</dt><dd>{confirmations}</dd></div><div><dt>Address</dt><dd className="break-all font-mono text-xs">{entry.address || "—"}</dd></div>
      <div className="col-span-2"><dt>Payment ID</dt><dd className="break-all font-mono text-xs">{entry.payment_id || "None"}</dd></div></dl>
    {showRaw ? <pre className="mt-4 overflow-x-auto rounded-md border border-slate-700 p-3 text-xs">{JSON.stringify(entry, null, 2)}</pre> : null}
    {entry.double_spend_seen ? <Alert message="The node reported a possible double spend for this transaction." /> : null}
    {entry.destinations.map((destination) => <div className="mt-3 flex items-start gap-3" key={destination.address}><div className="min-w-0 flex-1"><p className="break-all font-mono text-xs">{destination.address}</p><p className="mt-1">{formatRyo(destination.amount)} RYO</p></div><CopyButton value={destination.address} label="Copy address" /></div>)}
    <form className="mt-4 flex items-end gap-3" onSubmit={(event) => {
      event.preventDefault(); if (busy) return
      const note = String(new FormData(event.currentTarget).get("note") ?? ""); setBusy(true); setError(null)
      void walletOperation(generation, { type: "set_note", txid: entry.txid, note }).catch((cause: unknown) => setError(String(cause)))
        .finally(() => { void client.invalidateQueries({ queryKey: ["wallet-operation", generation, "history"] }); setBusy(false) })
    }}><div className="flex-1"><Field label="Transaction notes"><textarea name="note" defaultValue={entry.note} maxLength={2000} rows={2} className={inputClass} /></Field></div><Button type="submit" disabled={busy}>Save notes</Button></form><Alert message={error} />
  </div>
}

function WalletActions({ generation, action, name, onClose, onRemoved }: SessionProps & { action: string; name: string; onClose: () => void; onRemoved: () => void }) {
  const client = useQueryClient()
  const [busy, setBusy] = useState(false)
  const guard = useRef(false)
  const [error, setError] = useState<string | null>(null)
  const [message, setMessage] = useState<string | null>(null)
  const [secrets, setSecrets] = useState<SecretMaterial | null>(null)
  useEffect(() => { if (!secrets) return; const timer = window.setTimeout(() => setSecrets(null), 60_000); return () => window.clearTimeout(timer) }, [secrets])
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); if (guard.current) return
    const form = event.currentTarget; const values = new FormData(form)
    const password = String(values.get("password") ?? "")
    const newPassword = String(values.get("new_password") ?? "")
    if (action === "password" && newPassword !== values.get("confirmation")) { setError("Passwords do not match."); form.reset(); return }
    form.reset(); guard.current = true; setBusy(true); setError(null); setMessage(null)
    try {
      if (action === "name") {
        await walletOperation(generation, { type: "set_name", name: String(values.get("name") ?? "") }); setMessage("Wallet name saved.")
        await client.invalidateQueries({ queryKey: ["wallet-list"] })
        await client.invalidateQueries({ queryKey: ["active-wallet"] })
      }
      if (action === "password") { await walletOperation(generation, { type: "change_password", old_password: password, new_password: newPassword }); setMessage("Password changed.") }
      if (action === "secrets") { setSecrets(await walletOperation<SecretMaterial>(generation, { type: "secrets", password })) }
      if (action === "rescan") { await walletOperation(generation, { type: "rescan", spent_only: values.get("rescan") === "spent" }); setMessage("Rescan requested. The wallet sync will update below.") }
      if (action === "images") { const result = await manageKeyImages(generation, values.get("images") === "import", password); setMessage(result === null ? "File selection cancelled." : "Key image operation completed.") }
      if (action === "remove") { await removeWallet(generation, password, String(values.get("confirmation") ?? "")); onRemoved() }
      await client.invalidateQueries({ queryKey: ["wallet-operation", generation] }); await client.invalidateQueries({ queryKey: ["wallet-overview"] })
    } catch (cause) { setError(String(cause)) } finally { guard.current = false; setBusy(false) }
  }
  const titles: Record<string, string> = { name: "Rename wallet", password: "Change password", secrets: "Show private keys", rescan: "Rescan wallet", images: "Manage key images", remove: "Delete wallet" }
  return <section className={cardClass} aria-label={titles[action]}>
    <div className="flex justify-between"><h2 className="text-lg font-semibold">{titles[action]}</h2><Button variant="ghost" size="sm" disabled={busy} onClick={() => { setSecrets(null); onClose() }}>Close</Button></div>
    {secrets ? <div className="mt-4 grid gap-4">
      <p className="text-sm text-amber-200">These secrets control the wallet. This view hides automatically after one minute. Copied secrets remain in the system clipboard until replaced.</p>
      {([["Recovery phrase", secrets.phrase], ["Private view key", secrets.view_key], ["Private spend key", secrets.spend_key]] as const).map(([label, value]) => <div key={label}><p className="text-sm text-slate-400">{label}</p><p className="mt-1 break-all font-mono text-sm">{value}</p><div className="mt-2"><CopyButton value={value} label={`Copy ${label.toLowerCase()}`} /></div></div>)}
      <Button className="justify-self-start" onClick={() => setSecrets(null)}>Hide secrets</Button>
    </div> : <form onSubmit={(event) => void submit(event)} className="mt-4 grid gap-4 md:grid-cols-2">
      {action === "name" ? <Field label="Wallet name"><input name="name" required maxLength={100} defaultValue={name} className={inputClass} /></Field> : null}
      {action === "password" || action === "secrets" || action === "remove" || action === "images" ? <Field label="Current wallet password"><input name="password" type="password" autoComplete="current-password" required={action !== "images"} className={inputClass} /></Field> : null}
      {action === "password" ? <><Field label="New password"><input name="new_password" type="password" minLength={12} required autoComplete="new-password" className={inputClass} /></Field><Field label="Confirm new password"><input name="confirmation" type="password" required autoComplete="new-password" className={inputClass} /></Field></> : null}
      {action === "rescan" ? <><Field label="Rescan type"><select name="rescan" className={inputClass}><option value="spent">Spent outputs only</option><option value="full">Full blockchain scan from genesis</option></select></Field><p className="text-sm text-amber-200">A full scan can take time and removes previously stored outgoing transaction details. The downloaded chain remains available.</p><label className="flex gap-2 text-sm"><input type="checkbox" required />I understand and want to rescan.</label></> : null}
      {action === "images" ? <><Field label="Key image operation"><select name="images" className={inputClass}><option value="export">Export encrypted key images</option><option value="import">Import key images</option></select></Field><p className="text-sm text-slate-400">Export requires the wallet password. Choose the file in the native dialog.</p></> : null}
      {action === "remove" ? <><p className="text-sm text-amber-200">This removes the wallet from the app. Its encrypted files are kept in removed-wallets inside your data folder for recovery.</p><Field label="Type DELETE to confirm"><input name="confirmation" required pattern="DELETE" className={inputClass} /></Field><label className="flex gap-2 text-sm"><input type="checkbox" required />I have my recovery phrase backup.</label></> : null}
      <Button type="submit" disabled={busy} className="justify-self-start md:col-span-2">{busy ? "Working…" : action === "secrets" ? "Show secrets" : titles[action]}</Button>
    </form>}
    {message ? <p role="status" className="mt-4 text-sm text-emerald-200">{message}</p> : null}<Alert message={error} />
  </section>
}
