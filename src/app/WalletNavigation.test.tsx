// @vitest-environment jsdom
import { act } from "react"
import { createRoot, type Root } from "react-dom/client"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { WalletDashboard } from "./WalletDashboard"
import type { WalletOperation, WalletSection } from "@/api/operations"

vi.mock("@/api/overview", () => ({ getWalletOverview: async () => ({
  session_generation: "7", primary_address: "test-address", total: { atomic: "0" },
  unlocked: { atomic: "0" }, locked: { atomic: "0" }, multisig_import_needed: false,
}) }))
vi.mock("@/api/wallet", () => ({
  getReceiveAddresses: async () => [
    { address_index: 0, address: "test-address", label: "Primary account", used: false },
    { address_index: 1, address: "second-test-address", label: "Savings", used: true },
  ], createReceiveAddress: vi.fn(),
  getWalletSyncStatus: async () => ({ wallet_height: "1", daemon_height: "2", network_height: "3",
    node_reachable: true, node_ready: false, node_offline: false, node_untrusted: false }),
}))
vi.mock("@/api/operations", () => ({
  walletOperation: async (_generation: string, operation: WalletOperation) => operation.type === "info" ? { name: "Saved wallet" } : [],
  exportArtwork: vi.fn(), manageKeyImages: vi.fn(), removeWallet: vi.fn(),
}))

describe("wallet navigation", () => {
  let container: HTMLDivElement
  let root: Root
  let client: QueryClient

  beforeEach(() => {
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
    container = document.createElement("div")
    document.body.append(container)
    root = createRoot(container)
    client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity, staleTime: Infinity } } })
    client.setQueryData(["wallet-operation", "7", "info"], { name: "Saved wallet" })
    client.setQueryData(["receive-addresses", "7"], [
      { address_index: 0, address: "test-address", label: "Primary account", used: false },
      { address_index: 1, address: "second-test-address", label: "Savings", used: true },
    ])
  })
  afterEach(async () => {
    await act(async () => root.unmount())
    client.clear()
    container.remove()
  })

  async function show(section: WalletSection, generation = "7") {
    await act(async () => {
      root.render(<QueryClientProvider client={client}><WalletDashboard generation={generation} section={section}
        onSection={() => {}} onLock={() => {}} onRemoved={() => {}} locking={false} /></QueryClientProvider>)
    })
  }
  async function click(label: string) {
    const button = Array.from(container.querySelectorAll("button")).find((button) => button.textContent === label)
    expect(button, label).toBeDefined()
    await act(async () => button!.click())
  }
  async function openMenu() {
    const menu = container.querySelector("details")!
    await act(async () => menu.querySelector("summary")!.click())
    expect(menu.open).toBe(true)
  }

  it.each<WalletSection>(["receive", "send", "contacts", "history"])("closes the previous action and menu when switching to %s", async (section) => {
    await show("overview")
    await openMenu()
    await click("Rename wallet")
    expect(container.querySelector('section[aria-label="Rename wallet"]')).not.toBeNull()
    await openMenu()
    await show(section)
    expect(container.querySelector('section[aria-label="Rename wallet"]')).toBeNull()
    expect(container.querySelector("details")!.open).toBe(false)
  })

  it("discards the old form on return while preserving hidden balances across tabs", async () => {
    await show("overview")
    await click("Hide balances")
    await openMenu()
    await click("Rename wallet")
    container.querySelector<HTMLInputElement>('input[name="name"]')!.value = "Unsaved edit"
    await show("receive")
    await show("overview")
    expect(container.querySelector('section[aria-label="Rename wallet"]')).toBeNull()
    expect(container.textContent).toContain("Show balances")
    await openMenu()
    await click("Rename wallet")
    expect(container.querySelector<HTMLInputElement>('input[name="name"]')!.value).toBe("Saved wallet")
  })

  it("discards actions and the open menu on session changes", async () => {
    await show("overview")
    await openMenu()
    await click("Rename wallet")
    await openMenu()
    await show("overview", "8")
    expect(container.querySelector('section[aria-label="Rename wallet"]')).toBeNull()
    expect(container.querySelector("details")!.open).toBe(false)
  })

  it("shows the selected receive address once and updates its QR, label and request form on selection", async () => {
    await show("receive")
    expect(container.textContent!.split("test-address")).toHaveLength(2)
    const selection = container.querySelector('[aria-label="Receive address selection"]')!
    expect(selection.textContent).toContain("Primary account")
    expect(selection.textContent).toContain("Savings")
    expect(selection.textContent).not.toContain("test-address")
    const oldQr = container.querySelector('[aria-label="Receive QR code"]')!.innerHTML
    container.querySelector<HTMLInputElement>('input[name="amount"]')!.value = "12"
    await act(async () => selection.querySelectorAll("button")[1].click())
    expect(container.textContent!.split("second-test-address")).toHaveLength(2)
    expect(container.querySelector<HTMLInputElement>('input[name="label"]')!.value).toBe("Savings")
    expect(container.querySelector<HTMLInputElement>('input[name="amount"]')!.value).toBe("")
    expect(container.querySelector('[aria-label="Receive QR code"]')!.innerHTML).not.toBe(oldQr)
    expect(selection.querySelectorAll("button")[1].getAttribute("aria-pressed")).toBe("true")
  })
})
