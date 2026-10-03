import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import type { NodeConfig } from "@/api/generated/NodeConfig"
import { getNodeStatus, startNode, stopNode } from "@/api/node"

export function useNodeStatus(node: NodeConfig | null, root: string | null) {
  return useQuery({
    queryKey: ["node-status", root, node],
    queryFn: getNodeStatus,
    enabled: node !== null && root !== null,
    retry: false,
    refetchInterval: 2000,
    refetchIntervalInBackground: true,
  })
}

export function useNodeControl() {
  const client = useQueryClient()
  return useMutation({
    mutationKey: ["node-control"],
    mutationFn: (action: "start" | "stop") => action === "start" ? startNode() : stopNode(),
    onSettled: () => client.invalidateQueries({ queryKey: ["node-status"] }),
  })
}
