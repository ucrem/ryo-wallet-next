import { useState, type FormEvent } from "react"
import { useMutation, useQueryClient } from "@tanstack/react-query"
import type { NodeConfig } from "@/api/generated/NodeConfig"
import { saveNodeSelection, type NodeSelection } from "@/api/node"
import { Button } from "@/components/ui/button"

export function NodeSetup({ current, root, onSaved }: { current: NodeConfig | null; root: string; onSaved: () => void }) {
  const queryClient = useQueryClient()
  const [mode, setMode] = useState<"local" | "remote">(current?.mode ?? "local")
  const [host, setHost] = useState(current?.mode === "remote" ? current.host : "")
  const [port, setPort] = useState(current?.mode === "remote" ? String(current.port) : "12211")
  const save = useMutation({
    mutationFn: saveNodeSelection,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["node-configuration", root] })
      onSaved()
    },
  })
  const parsedPort = Number(port)
  const remoteValid =
    host.trim().length > 0 && /^\d+$/.test(port) && parsedPort >= 1 && parsedPort <= 65535

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const selection: NodeSelection =
      mode === "local" ? { mode: "local" } : { mode: "remote", host: host.trim(), port: parsedPort }
    save.mutate(selection)
  }

  return (
    <form onSubmit={submit} className="grid gap-3">
      {current ? (
        <p className="text-sm text-slate-300" role="status">
          Saved: {current.mode === "local" ? "local node" : `${current.host}:${current.port}`}
        </p>
      ) : null}
      <fieldset className="grid gap-2 sm:grid-cols-2">
        <legend className="mb-2 text-sm font-medium">Node mode · Mainnet</legend>
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
          <span><strong>Local</strong> · Use your own daemon on this computer.</span>
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
      {mode === "remote" ? (
        <div className="grid gap-3 sm:grid-cols-[1fr_8rem]">
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
              className="min-w-0 rounded-md border border-slate-600 bg-[#0d141c] px-3 py-2"
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
              className="min-w-0 rounded-md border border-slate-600 bg-[#0d141c] px-3 py-2"
            />
          </label>
          <p className="text-xs leading-5 text-amber-200 sm:col-span-2">
            When connected, a remote node can see your IP address and wallet scan requests. The planned RPC connection uses plain HTTP.
          </p>
        </div>
      ) : null}
      <div className="flex flex-wrap items-center gap-3">
        <Button type="submit" className="bg-sky-400 text-slate-950 hover:bg-sky-300" disabled={save.isPending || (mode === "remote" && !remoteValid)}>
          {save.isPending ? "Saving…" : "Save and continue →"}
        </Button>
        {save.isError ? (
          <p className="text-sm text-red-300" role="alert">
            The node choice is invalid or could not be saved.
          </p>
        ) : null}
      </div>
    </form>
  )
}
