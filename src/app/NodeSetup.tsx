import { useState, type FormEvent } from "react"
import { useMutation, useQueryClient } from "@tanstack/react-query"
import type { NodeConfig } from "@/api/generated/NodeConfig"
import { saveNodeSelection, type NodeSelection } from "@/api/node"
import { Button } from "@/components/ui/button"

export function NodeSetup({ current, root, onSaved, disabled = false }: { current: NodeConfig | null; root: string; onSaved: () => void; disabled?: boolean }) {
  const queryClient = useQueryClient()
  const [mode, setMode] = useState<"local" | "remote" | "hybrid">(current?.mode ?? "local")
  const [host, setHost] = useState(current?.mode === "remote" ? current.host : current?.bootstrap?.host ?? "")
  const [port, setPort] = useState(current?.mode === "remote" ? String(current.port) : String(current?.bootstrap?.port ?? (current?.network === "testnet" ? 13311 : 12211)))
  const save = useMutation({
    mutationFn: saveNodeSelection,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["node-configuration", root] })
      await queryClient.invalidateQueries({ queryKey: ["node-status"] })
      onSaved()
    },
  })
  const parsedPort = Number(port)
  const remoteValid =
    host.trim().length > 0 && /^\d+$/.test(port) && parsedPort >= 1 && parsedPort <= 65535
  const usesSavedNode = current !== null && mode === current.mode &&
    (mode === "local" || (remoteValid && host.trim() === (mode === "hybrid" ? current.bootstrap?.host : current.host) && parsedPort === (mode === "hybrid" ? current.bootstrap?.port : current.port)))

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (save.isPending) return
    if (usesSavedNode) {
      onSaved()
      return
    }
    if (disabled || (mode !== "local" && !remoteValid)) return
    const selection: NodeSelection =
      mode === "local" ? { mode: "local" } : { mode, host: host.trim(), port: parsedPort }
    save.mutate(selection)
  }

  return (
    <form onSubmit={submit} className="grid gap-3">
      {current ? (
        <p className="text-sm text-slate-300" role="status">
          Saved: {current.mode === "hybrid" ? `local node with bootstrap ${current.bootstrap?.host}:${current.bootstrap?.port}` : current.mode === "local" ? "local node" : `${current.host}:${current.port}`}
        </p>
      ) : null}
      <fieldset disabled={disabled || save.isPending} className="grid gap-2 sm:grid-cols-3">
        <legend className="mb-2 text-sm font-medium">Node mode · {(current?.network ?? "mainnet").replace(/^./, (letter) => letter.toUpperCase())}</legend>
        <label className="flex cursor-pointer items-start gap-3 rounded-lg border border-slate-600 p-3 text-sm">
          <input type="radio" name="node-mode" value="hybrid" checked={mode === "hybrid"} onChange={() => { setMode("hybrid"); save.reset() }} className="mt-1" />
          <span><strong>Local + Remote</strong> · Sync locally with a remote bootstrap node until your chain catches up.</span>
        </label>
        <label className="flex cursor-pointer items-start gap-3 rounded-lg border border-slate-600 p-3 text-sm">
          <input
            type="radio"
            name="node-mode"
            value="local"
            checked={mode === "local"}
            onChange={() => {
              setMode("local")
              save.reset()
            }}
            className="mt-1"
          />
          <span><strong>Local</strong> · Download the blockchain with the bundled node. It stays active while the wallet is locked.</span>
        </label>
        <label className="flex cursor-pointer items-start gap-3 rounded-lg border border-slate-600 p-3 text-sm">
          <input
            type="radio"
            name="node-mode"
            value="remote"
            checked={mode === "remote"}
            onChange={() => {
              setMode("remote")
              save.reset()
            }}
            className="mt-1"
          />
          <span><strong>Remote</strong> · Use a node whose host and port you choose.</span>
        </label>
      </fieldset>
      {mode !== "local" ? (
        <fieldset disabled={disabled || save.isPending} className="grid gap-3 sm:grid-cols-[1fr_8rem]">
          <label className="grid gap-1 text-sm">
            <span>Host</span>
            <input
              type="text"
              value={host}
              onChange={(event) => {
                setHost(event.target.value)
                save.reset()
              }}
              placeholder="node.example.org"
              autoComplete="off"
              required
              className="min-w-0 rounded-md border border-slate-600 bg-[var(--app-input)] px-3 py-2"
            />
          </label>
          <label className="grid gap-1 text-sm">
            <span>RPC port</span>
            <input
              type="number"
              value={port}
              onChange={(event) => {
                setPort(event.target.value)
                save.reset()
              }}
              min="1"
              max="65535"
              required
              className="min-w-0 rounded-md border border-slate-600 bg-[var(--app-input)] px-3 py-2"
            />
          </label>
          <p className="text-xs leading-5 text-amber-200 sm:col-span-2">
            When connected, a remote node can see your IP address and wallet scan requests. The RPC connection uses plain HTTP.
          </p>
        </fieldset>
      ) : null}
      <div className="flex flex-wrap items-center gap-3">
        <Button type="submit" className="bg-sky-400 text-slate-950 hover:bg-sky-300" disabled={(disabled && !usesSavedNode) || save.isPending || (mode !== "local" && !remoteValid)}>
          {save.isPending ? "Saving…" : usesSavedNode ? "Continue →" : "Save and continue →"}
        </Button>
        {save.isError ? (
          <p className="text-sm text-red-300" role="alert">
            The node choice is invalid or could not be saved.
          </p>
        ) : null}
      </div>
      {disabled ? <p className="text-xs text-slate-400">To change node settings, lock the wallet and stop the local node. You can continue with the saved configuration.</p> : null}
      {mode === "local" ? <p className="text-xs text-slate-400">The blockchain uses disk space and network bandwidth. Start or stop the node from the status bar, including while the wallet is locked.</p> : null}
    </form>
  )
}
