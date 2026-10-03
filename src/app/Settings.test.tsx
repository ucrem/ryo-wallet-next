// @vitest-environment jsdom
import { act } from "react"
import { createRoot, type Root } from "react-dom/client"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { Settings, generalDraft } from "./Settings"
import { defaultNodeOptions, defaultPreferences } from "@/api/settings"
import type { NodeConfig } from "@/api/generated/NodeConfig"
import { SendReview } from "./WalletDashboard"

const mocks = vi.hoisted(() => ({ savePreferences: vi.fn(), saveGeneralSettings: vi.fn() }))
vi.mock("@/api/settings", async (original) => ({ ...await original<typeof import("@/api/settings")>(), ...mocks }))

const node: NodeConfig = { mode: "local", network: "mainnet", host: "127.0.0.1", port: 12211, trust: "managed_local" }
describe("settings and notifications", () => {
  let container: HTMLDivElement, root: Root, client: QueryClient
  beforeEach(() => {
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
    container = document.createElement("div"); document.body.append(container); root = createRoot(container)
    client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity } } })
    client.setQueryData(["app-preferences"], defaultPreferences)
    mocks.savePreferences.mockReset().mockImplementation(async (value) => value)
    mocks.saveGeneralSettings.mockReset().mockResolvedValue(node)
  })
  afterEach(async () => { await act(async () => root.unmount()); client.clear(); container.remove() })
  async function show(busy: boolean) {
    await act(async () => root.render(<QueryClientProvider client={client}><Settings root="G:\\" node={node} network="mainnet" busy={busy} /></QueryClientProvider>))
  }
  const button = (label: string) => Array.from(container.querySelectorAll("button")).find((entry) => entry.textContent === label)!

  it("blocks general changes during sync but saves only preferences without restarting the node", async () => {
    await show(true)
    expect(button("Save").disabled).toBe(true)
    await act(async () => button("Preferences").click())
    const checkbox = Array.from(container.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')).find((entry) => entry.parentElement!.textContent!.includes("Payment ID"))!
    await act(async () => checkbox.click())
    await act(async () => button("Save").click())
    expect(mocks.savePreferences).toHaveBeenCalledWith({ ...defaultPreferences, notify_no_payment_id: false }, expect.anything())
    expect(mocks.saveGeneralSettings).not.toHaveBeenCalled()
  })

  it("hydrates hybrid settings with their remote bootstrap and advanced limits", () => {
    const draft = generalDraft({ ...node, mode: "hybrid", bootstrap: { host: "example.org", port: 13311 }, advanced: { ...defaultNodeOptions, limit_rate_down: 512 } }, "mainnet")
    expect(draft.host).toBe("example.org"); expect(draft.port).toBe(13311)
    expect(draft.advanced.limit_rate_down).toBe(512)
  })

  it("requires separate acknowledgement for a missing Payment ID when the warning is enabled", async () => {
    const confirm = vi.fn()
    await act(async () => root.render(<SendReview draft={{ token: "test", address: "recipient", payment_id: "", amount: "1", fee: "1", total: "2", transactions: [], expires_in_seconds: 300 }} busy={false} warnNoPaymentId onConfirm={confirm} onCancel={() => {}} />))
    const checks = container.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')
    await act(async () => checks[1].click())
    expect(button("Confirm and send").disabled).toBe(true)
    await act(async () => checks[0].click())
    expect(button("Confirm and send").disabled).toBe(false)
    await act(async () => button("Confirm and send").click())
    expect(confirm).toHaveBeenCalledOnce()
  })
})
