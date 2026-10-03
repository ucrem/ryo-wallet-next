import type { NodeConfig } from "@/api/generated/NodeConfig"
import { useIsMutating } from "@tanstack/react-query"
import { useNodeControl } from "@/lib/useNodeStatus"
import { Button } from "@/components/ui/button"

export function NodeControls({ node, running, walletActive, compact = false }: { node: NodeConfig | null; running: boolean; walletActive: boolean; compact?: boolean }) {
  const control = useNodeControl()
  const pending = useIsMutating({ mutationKey: ["node-control"] }) > 0
  if (!node || node.mode === "remote") return null
  return <div className="flex items-center gap-2">
    <span title={walletActive ? "Lock the wallet to control the node." : undefined}>
      <Button type="button" variant="outline" size={compact ? "xs" : "sm"} disabled={pending || walletActive}
        onClick={() => control.mutate(running ? "stop" : "start")}>
        {pending ? "Please wait…" : running ? "Stop node" : "Start node"}
      </Button>
    </span>
    {walletActive ? <span className={compact ? "sr-only" : "text-xs text-slate-400"}>Lock the wallet to control the node.</span> : null}
    {control.isError ? <span role="alert" title={String(control.error)} className="text-xs text-red-300">{compact ? "Node action failed" : String(control.error)}</span> : null}
  </div>
}
