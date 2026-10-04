// @vitest-environment jsdom
import { act } from "react"
import { createRoot, type Root } from "react-dom/client"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import type { NodeConfig } from "@/api/generated/NodeConfig"
import type { Network } from "@/api/generated/Network"
import { defaultPreferences } from "@/api/settings"
import { Settings } from "./Settings"
import { NodeSetup } from "./NodeSetup"

const mocks = vi.hoisted(() => ({ saveNodeSelection: vi.fn(), saveGeneralSettings: vi.fn() }))
vi.mock("@/api/node", async (original) => ({ ...await original<typeof import("@/api/node")>(), saveNodeSelection: mocks.saveNodeSelection }))
vi.mock("@/api/settings", async (original) => ({ ...await original<typeof import("@/api/settings")>(), saveGeneralSettings: mocks.saveGeneralSettings }))

describe.each(["setup", "settings"] as const)("remote suggestion in %s", (surface) => {
  let container: HTMLDivElement, root: Root, client: QueryClient
  beforeEach(() => {
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
    container = document.createElement("div"); document.body.append(container); root = createRoot(container)
    client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity } } })
    client.setQueryData(["app-preferences"], defaultPreferences)
    mocks.saveNodeSelection.mockReset().mockResolvedValue(null)
    mocks.saveGeneralSettings.mockReset().mockResolvedValue(null)
  })
  afterEach(async () => { await act(async () => root.unmount()); client.clear(); container.remove() })

  async function show(network: Network = "mainnet", busy = false, saved = true) {
    const node: NodeConfig | null = saved ? { mode: "hybrid", network, host: "127.0.0.1", port: 12211, trust: "managed_local", bootstrap: { host: "my-node.example.org", port: 18081 } } : null
    await act(async () => root.render(<QueryClientProvider client={client}>
      {surface === "setup" ? <NodeSetup current={node} network={network} root="test-root" disabled={busy} onSaved={() => {}} />
        : <Settings node={node} network={network} root="test-root" busy={busy} />}
    </QueryClientProvider>))
  }
  const button = (label: string) => Array.from(container.querySelectorAll("button")).find((entry) => entry.textContent === label)!
  const endpoint = () => Array.from(container.querySelectorAll<HTMLInputElement>("input")).filter((entry) => entry.type !== "radio").slice(0, 2).map((entry) => entry.value)

  it("preserves the custom endpoint, fills both fields on request and saves only on submit", async () => {
    await show()
    expect(endpoint()).toEqual(["my-node.example.org", "18081"])
    await act(async () => button("Use suggestion").click())
    expect(endpoint()).toEqual(["wallet-node.ryo-currency.com", "12211"])
    expect(mocks.saveNodeSelection).not.toHaveBeenCalled()
    expect(mocks.saveGeneralSettings).not.toHaveBeenCalled()
    await act(async () => button(surface === "setup" ? "Save and continue →" : "Save").click())
    expect(surface === "setup" ? mocks.saveNodeSelection : mocks.saveGeneralSettings).toHaveBeenCalledWith(expect.objectContaining({ mode: "hybrid", host: "wallet-node.ryo-currency.com", port: 12211 }), expect.anything())
  })

  it("keeps the suggestion locked while node settings are in use", async () => {
    await show("mainnet", true)
    expect(button("Use suggestion").matches(":disabled")).toBe(true)
    await act(async () => button("Use suggestion").click())
    expect(endpoint()).toEqual(["my-node.example.org", "18081"])
  })

  it.each(["testnet", "stagenet"] as const)("does not suggest a mainnet RPC for %s", async (network) => {
    await show(network)
    expect(container.textContent).not.toContain("Use suggestion")
    expect(container.innerHTML).not.toContain("wallet-node.ryo-currency.com")
    expect(endpoint()).toEqual(["my-node.example.org", "18081"])
  })

  it("uses the selected test network before a node configuration has been saved", async () => {
    await show("testnet", false, false)
    await act(async () => container.querySelectorAll<HTMLInputElement>('input[type="radio"]')[2].click())
    expect(endpoint()).toEqual(["", "13311"])
    expect(container.innerHTML).not.toContain("wallet-node.ryo-currency.com")
  })
})
