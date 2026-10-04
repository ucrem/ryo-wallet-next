import type { Network } from "@/api/generated/Network"
import { remoteNodeSuggestion } from "@/lib/remoteNodeSuggestion"
import { Button } from "@/components/ui/button"

export function RemoteNodeSuggestion({ network, onSelect, className = "" }: { network: Network; onSelect: (host: string, port: number) => void; className?: string }) {
  const suggestion = remoteNodeSuggestion(network)
  if (!suggestion) return null

  return <div className={`flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-slate-400 ${className}`}>
    <p title={`Source: ${suggestion.source}`}>Ryo Atom mainnet suggestion: <span className="font-mono text-slate-300">{suggestion.host}:{suggestion.port}</span></p>
    <Button type="button" variant="outline" size="xs" onClick={() => onSelect(suggestion.host, suggestion.port)}>Use suggestion</Button>
  </div>
}
