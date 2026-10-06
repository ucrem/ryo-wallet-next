import { useState, type ReactNode } from "react"
import { useMutation, useQueryClient } from "@tanstack/react-query"
import type { NodeConfig } from "@/api/generated/NodeConfig"
import type { Network } from "@/api/generated/Network"
import { chooseDataRoot } from "@/api/onboarding"
import { defaultNodeOptions, defaultPreferences, saveGeneralSettings, savePreferences, type GeneralSettings, type Preferences } from "@/api/settings"
import { usePreferences } from "@/lib/usePreferences"
import { formatDataFolderPath } from "@/lib/displayPath"
import { Button } from "@/components/ui/button"
import { RemoteNodeSuggestion } from "@/components/RemoteNodeSuggestion"
import { defaultNodeRpcPort, remoteNodeSuggestion } from "@/lib/remoteNodeSuggestion"

const input = "w-full min-w-0 rounded-md border border-slate-600 bg-[var(--app-input)] px-3 py-1.5 text-sm disabled:opacity-50"
const panel = "rounded-xl border border-slate-700 bg-[var(--app-surface)] p-4"

export function generalDraft(node: NodeConfig | null, network: Network): GeneralSettings {
  return { mode: node?.mode ?? "local", network: node?.network ?? network,
    host: node?.mode === "remote" ? node.host : node?.bootstrap?.host ?? "",
    port: node?.mode === "remote" ? node.port : node?.bootstrap?.port ?? defaultNodeRpcPort(node?.network ?? network),
    advanced: { ...defaultNodeOptions, ...node?.advanced } }
}

export function Settings({ root, node, network, busy }: { root: string | null; node: NodeConfig | null; network: Network; busy: boolean }) {
  const client = useQueryClient()
  const prefs = usePreferences()
  const [tab, setTab] = useState<"general" | "preferences">("general")
  const [general, setGeneral] = useState(() => generalDraft(node, network))
  const [preferenceDraft, setPreferenceDraft] = useState<Preferences | null>(null)
  const preferences = preferenceDraft ?? prefs.data ?? defaultPreferences
  const [message, setMessage] = useState("")
  const generalSave = useMutation({ mutationFn: saveGeneralSettings, onSuccess: async () => {
    client.removeQueries({ predicate: (query) => /^(wallet-|active-wallet|receive-addresses|send-contact)/.test(String(query.queryKey[0])) })
    await Promise.all([client.invalidateQueries({ queryKey: ["node-configuration"] }), client.invalidateQueries({ queryKey: ["data-root-configured"] }), client.invalidateQueries({ queryKey: ["node-status"] })])
    setMessage("General settings saved. Start the node when you are ready.")
  } })
  const preferenceSave = useMutation({ mutationFn: savePreferences, onSuccess: (saved) => {
    client.setQueryData(["app-preferences"], saved); setPreferenceDraft(null); setMessage("Preferences saved.")
  } })
  const picker = useMutation({ mutationFn: chooseDataRoot, onSuccess: async (selected) => {
    if (selected) {
      await Promise.all([client.invalidateQueries({ queryKey: ["data-root-configured"] }), client.invalidateQueries({ queryKey: ["node-configuration"] })])
    }
  } })
  const pending = generalSave.isPending || preferenceSave.isPending || picker.isPending
  const error = generalSave.error ?? preferenceSave.error ?? picker.error
  const patch = (update: Partial<Preferences>) => { setPreferenceDraft({ ...preferences, ...update }); setMessage(""); preferenceSave.reset() }
  const options = general.advanced
  const local = general.mode !== "remote"
  const numericFields = [
    ["daemon_log_level", "Node log level", 0, 4], ["wallet_log_level", "Wallet log level", 0, 4],
    ["in_peers", "Maximum incoming peers", -1, 65535], ["out_peers", "Maximum outgoing peers", -1, 65535],
    ["limit_rate_up", "Upload limit (kB/s)", -1, 65535], ["limit_rate_down", "Download limit (kB/s)", -1, 65535],
    ["p2p_port", "Node P2P port", 0, 65535], ["rpc_port", "Node RPC port", 0, 65535],
    ["zmq_port", "Node ZMQ port", 0, 65535], ["wallet_rpc_port", "Wallet RPC port", 0, 65535],
  ] as const

  return <div>
    <div className="mb-4 flex items-center justify-between gap-4">
      <h1 className="text-2xl font-semibold tracking-tight">Settings</h1>
      <nav aria-label="Settings sections" className="flex rounded-lg border border-slate-700 p-1">
        {(["general", "preferences"] as const).map((section) => <button key={section} type="button" aria-current={tab === section ? "page" : undefined}
          onClick={() => { setTab(section); setMessage(""); generalSave.reset(); preferenceSave.reset() }}
          className={`rounded-md px-5 py-1.5 text-sm ${tab === section ? "bg-sky-400/10 font-medium text-sky-200" : "text-slate-400 hover:bg-slate-800"}`}>{section === "general" ? "General" : "Preferences"}</button>)}
      </nav>
      <Button type="submit" form="settings-form" size="sm" disabled={pending || (tab === "general" ? busy || !root : prefs.isPending || prefs.isError)} className="bg-sky-400 text-slate-950 hover:bg-sky-300">{pending ? "Saving…" : "Save"}</Button>
    </div>
    <form id="settings-form" onSubmit={(event) => {
      event.preventDefault(); if (pending) return
      setMessage("")
      if (tab === "general") { if (!busy && root) generalSave.mutate(general) }
      else if (prefs.data) preferenceSave.mutate(preferences)
    }}>
      {tab === "general" ? <section className={panel}>
        <fieldset disabled={busy || pending} className="grid gap-4">
          <legend className="mb-3 text-sm font-medium">Node connection</legend>
          <div className="grid gap-3 md:grid-cols-3">
            {([["hybrid", "Local + Remote"], ["local", "Local only"], ["remote", "Remote only"]] as const).map(([mode, label]) =>
              <label key={mode} className={`flex cursor-pointer items-center gap-3 rounded-lg border p-3 text-sm ${general.mode === mode ? "border-sky-500 bg-sky-400/5" : "border-slate-600"}`}>
                <input type="radio" name="node-mode" checked={general.mode === mode} onChange={() => { setGeneral({ ...general, mode }); setMessage(""); generalSave.reset() }} />{label}
              </label>)}
          </div>
          <p className="text-sm text-slate-400">{general.mode === "hybrid" ? "Download the chain locally and use your selected remote node while the local node catches up. Ryo switches to the local chain automatically."
            : local ? "Download and verify the blockchain locally. Transactions require a synchronized node and wallet." : "Connect the wallet to the remote node you select."}</p>
          {general.mode !== "local" ? <div className="grid gap-3 md:grid-cols-[1fr_10rem]">
            <Field label="Remote node host"><input required autoComplete="off" placeholder={remoteNodeSuggestion(general.network)?.host ?? "node.example.org"} value={general.host} onChange={(event) => setGeneral({ ...general, host: event.target.value })} className={input} /></Field>
            <Field label="Remote RPC port"><input required type="number" min={1} max={65535} value={general.port} onChange={(event) => setGeneral({ ...general, port: Number(event.target.value) })} className={input} /></Field>
            <RemoteNodeSuggestion network={general.network} className="md:col-span-2" onSelect={(host, port) => {
              setGeneral({ ...general, host, port }); setMessage(""); generalSave.reset()
            }} />
            <p className="text-xs text-amber-200 md:col-span-2">The remote node can see your IP and scan requests. Its RPC connection uses plain HTTP.</p>
          </div> : null}
          {local ? <div className="grid gap-3 md:grid-cols-2">
            <Field label="Local RPC address"><input readOnly value={`127.0.0.1 · port ${options.rpc_port || "automatic"}`} className={input} /></Field>
            <Field label="Local peer connections"><select value={options.public_p2p ? "public" : "private"} onChange={(event) => setGeneral({ ...general, advanced: { ...options, public_p2p: event.target.value === "public" } })} className={input}>
              <option value="private">Private mode · outbound connections only</option><option value="public">Interconnected mode · accept incoming peers</option>
            </select></Field>
          </div> : null}
        </fieldset>
        <div className="mt-4 flex items-end gap-3">
          <div className="min-w-0 flex-1"><Field label="Data storage folder"><input readOnly value={root ? formatDataFolderPath(root) : "No folder selected"} className={input} /></Field></div>
          <Button type="button" size="sm" variant="outline" disabled={busy || pending} onClick={() => picker.mutate()}>Select location</Button>
        </div>
        {busy ? <p className="mt-3 text-xs text-slate-400">Lock the wallet and stop the local node to change General settings.</p> : null}
        <details className="mt-4 border-t border-slate-700 pt-3">
          <summary className="cursor-pointer text-sm font-medium">Advanced options</summary>
          <fieldset disabled={busy || pending} className="mt-3 grid gap-3 md:grid-cols-4">
            <Field label="Network"><select value={general.network} onChange={(event) => {
              const next = event.target.value as Network
              setGeneral({ ...general, network: next, port: general.port === defaultNodeRpcPort(general.network) ? defaultNodeRpcPort(next) : general.port })
            }} className={input}><option value="mainnet">Mainnet</option><option value="testnet">Testnet</option><option value="stagenet">Stagenet</option></select></Field>
            {numericFields.map(([key, label, min, max]) => <Field key={key} label={label}>
              <input type="number" required min={min} max={max} step={1} disabled={!local && key !== "wallet_log_level" && key !== "wallet_rpc_port"} value={options[key]}
                onChange={(event) => setGeneral({ ...general, advanced: { ...options, [key]: Number(event.target.value) } })} className={input} />
            </Field>)}
            <p className="text-xs text-slate-400 md:col-span-4">Limits −1 use Ryo defaults. Port 0 selects an automatic private port; fixed ports must be distinct and at least 1024. Each network has separate wallets and blockchain data. Selecting a folder does not move existing files.</p>
          </fieldset>
        </details>
      </section> : <div className="grid gap-4 md:grid-cols-2">
        <section className={panel}><h2 className="font-semibold">Appearance</h2>
          <fieldset disabled={pending || prefs.isPending || prefs.isError} className="mt-3 flex gap-2">
            {(["light", "dark", "system"] as const).map((theme) => <label key={theme} className={`flex cursor-pointer items-center gap-2 rounded-md border px-4 py-2 text-sm ${preferences.theme === theme ? "border-sky-500 bg-sky-400/10" : "border-slate-600"}`}>
              <input type="radio" name="theme" checked={preferences.theme === theme} onChange={() => patch({ theme })} />{theme === "light" ? "Light" : theme === "dark" ? "Dark" : "System"}</label>)}
          </fieldset>
          <h2 className="mt-6 font-semibold">Notifications</h2>
          <div className="mt-3 grid gap-3 text-sm">
            <Check disabled={pending || !prefs.data} checked={preferences.notify_no_payment_id} onChange={(value) => patch({ notify_no_payment_id: value })}>Warn when sending without a Payment ID</Check>
            <Check disabled={pending || !prefs.data} checked={preferences.notify_weak_password} onChange={(value) => patch({ notify_weak_password: value })}>Warn about a weak password when creating or restoring a wallet</Check>
            <p className="text-xs text-slate-400">Warnings appear in the relevant form. Wallet passwords must still meet the minimum length.</p>
          </div>
        </section>
        <section className={panel}><h2 className="font-semibold">Desktop behavior</h2>
          <div className="mt-3 grid gap-3 text-sm">
            <Check disabled={pending || !prefs.data} checked={preferences.minimize_to_tray} onChange={(value) => patch({ minimize_to_tray: value })}>Minimize to tray</Check>
            <p className="text-xs text-slate-400">Closing or minimizing hides the window and keeps the node active. Use the tray menu to reopen, lock the wallet or exit.</p>
            <Check disabled={pending || !prefs.data} checked={preferences.launch_on_startup} onChange={(value) => patch({ launch_on_startup: value })}>Launch Ryo Wallet Next at startup</Check>
            <p className="text-xs text-slate-400">Startup opens the app with the wallet locked.</p>
          </div>
          <div className="mt-6"><Field label="Inactivity timeout">
            <select disabled={pending || !prefs.data} value={preferences.idle_lock_seconds} onChange={(event) => patch({ idle_lock_seconds: Number(event.target.value) })} className={input}>
              {[0, 60, 300, 600, 900, 1800, 3600, preferences.idle_lock_seconds].filter((value, index, values) => values.indexOf(value) === index).sort((a,b) => a-b).map((seconds) => <option key={seconds} value={seconds}>{seconds === 0 ? "Never" : `${seconds / 60} minute${seconds === 60 ? "" : "s"}`}</option>)}
            </select>
          </Field><p className="mt-2 text-xs text-slate-400">Automatically locks open wallets after no keyboard or pointer activity. The local node keeps syncing.</p></div>
        </section>
        {prefs.isError ? <p role="alert" className="text-sm text-red-300">Preferences could not be read. <button type="button" onClick={() => void prefs.refetch()} className="underline">Retry</button></p> : null}
      </div>}
    </form>
    {message ? <p role="status" className="mt-3 text-sm text-emerald-300">{message}</p> : null}
    {error ? <p role="alert" className="mt-3 text-sm text-red-300">{String(error)}</p> : null}
  </div>
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return <label className="grid gap-1.5 text-sm"><span className="text-slate-300">{label}</span>{children}</label>
}
function Check({ checked, onChange, children, disabled }: { checked: boolean; onChange: (value: boolean) => void; children: ReactNode; disabled: boolean }) {
  return <label className="flex items-start gap-2"><input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} disabled={disabled} className="mt-1" /><span>{children}</span></label>
}
