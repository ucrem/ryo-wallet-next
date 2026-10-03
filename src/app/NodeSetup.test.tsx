import { renderToStaticMarkup } from "react-dom/server"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { describe, expect, it } from "vitest"
import type { NodeConfig } from "@/api/generated/NodeConfig"
import { NodeSetup } from "./NodeSetup"

function render(current: NodeConfig | null) {
  return renderToStaticMarkup(<QueryClientProvider client={new QueryClient()}>
    <NodeSetup current={current} root="test-root" disabled onSaved={() => {}} />
  </QueryClientProvider>)
}

describe("continuing setup while node settings are in use", () => {
  it("allows continuing with a saved node while its editable fields stay locked", () => {
    for (const node of [
      { mode: "local", network: "mainnet", host: "127.0.0.1", port: 12211, trust: "managed_local" },
      { mode: "remote", network: "mainnet", host: "node.example.org", port: 12211, trust: "explicit_remote" },
    ] satisfies NodeConfig[]) {
      const markup = render(node)
      expect(markup).toMatch(/<fieldset[^>]*disabled=""/)
      expect(markup).toMatch(/<button(?![^>]*disabled=)[^>]*>Continue →<\/button>/)
    }
  })

  it("blocks saving a new selection while settings are in use", () => {
    expect(render(null)).toMatch(/<button[^>]*disabled=""[^>]*>Save and continue →<\/button>/)
  })
})
