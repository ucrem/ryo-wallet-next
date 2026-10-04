// @vitest-environment jsdom
import { act } from "react"
import { createRoot, type Root } from "react-dom/client"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import type { LifecycleStatus } from "@/api/generated/LifecycleStatus"
import { defaultPreferences } from "@/api/settings"
import { getBackupPhrase, listWallets, openWallet, lockWallet, createWallet, restoreWallet, importWallet } from "@/api/wallet"
import { App } from "./App"

const state = vi.hoisted(() => ({ lifecycle: { state: "locked", session_generation: "7" } as LifecycleStatus, listeners: new Map<string, (event: { payload: LifecycleStatus }) => void>() }))
vi.mock("@tauri-apps/api/event", () => ({ listen: async (name: string, callback: (event: { payload: LifecycleStatus }) => void) => {
  state.listeners.set(name, callback); return () => { state.listeners.delete(name) }
} }))
vi.mock("@/api/status", () => ({ getFoundationStatus: async () => state.lifecycle }))
vi.mock("@/api/onboarding", () => ({ getDataRootConfiguration: async () => ({ root: "test-root", network: "mainnet" }), chooseDataRoot: vi.fn() }))
vi.mock("@/api/node", () => ({ getNodeConfiguration: async () => ({ mode: "remote", network: "mainnet", host: "node.example.org", port: 12211, trust: "explicit_remote" }) }))
vi.mock("@/api/overview", () => ({ getWalletOverview: async () => null }))
vi.mock("@/api/settings", async (original) => ({ ...await original<typeof import("@/api/settings")>(), getPreferences: async () => defaultPreferences, recordActivity: async () => {} }))
vi.mock("@/api/wallet", () => ({ listWallets: vi.fn(), openWallet: vi.fn(), lockWallet: vi.fn(), getActiveWallet: async () => null,
  walletRuntimeReady: async () => true, getBackupPhrase: vi.fn(), acknowledgeBackup: vi.fn(),
  selectImportWallet: vi.fn(), importWallet: vi.fn(), createWallet: vi.fn(), restoreWallet: vi.fn() }))
vi.mock("@/lib/useNodeStatus", () => ({ useNodeStatus: () => ({ data: { state: "running" }, isPending: false, isError: false }) }))
vi.mock("@/lib/useAppVersion", () => ({ useAppVersion: () => ({ version: "0.1.0-alpha.6", label: "v0.1.0-alpha.6", isNative: true }) }))
vi.mock("@/lib/useAppUpdates", () => ({ useAppUpdates: () => ({ result: null }) }))
vi.mock("./WalletStatusBar", () => ({ WalletStatusBar: () => null }))
vi.mock("./WalletDashboard", () => ({ WalletDashboard: ({ onLock }: { onLock: () => void }) => <div>Saved wallet dashboard<button type="button" onClick={onLock}>Lock</button></div> }))

const wallets = [{ id: "a".repeat(32), backup_complete: true }, { id: "b".repeat(32), backup_complete: true }]
describe("Home saved wallets", () => {
  let container: HTMLDivElement, root: Root, client: QueryClient
  beforeEach(() => {
    vi.resetAllMocks(); state.listeners.clear(); state.lifecycle = { state: "locked", session_generation: "7" }
    vi.mocked(listWallets).mockResolvedValue(wallets)
    vi.mocked(openWallet).mockImplementation(async () => { state.lifecycle = { state: "open", session_generation: "8" }; return state.lifecycle })
    vi.mocked(lockWallet).mockImplementation(async () => { state.lifecycle = { state: "locked", session_generation: "9" }; return state.lifecycle })
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
    Object.assign(window, { __TAURI_INTERNALS__: {}, matchMedia: () => ({ matches: false, addEventListener: () => {}, removeEventListener: () => {} }) })
    container = document.createElement("div"); document.body.append(container); root = createRoot(container)
    client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity, staleTime: Infinity } } })
    client.setQueryData(["foundation-status"], state.lifecycle)
    client.setQueryData(["data-root-configured"], { root: "test-root", network: "mainnet" })
    client.setQueryData(["node-configuration", "test-root"], { mode: "remote", network: "mainnet", host: "node.example.org", port: 12211, trust: "explicit_remote" })
    client.setQueryData(["wallet-list", "test-root", "mainnet"], wallets)
    client.setQueryData(["wallet-runtime-ready"], true)
    client.setQueryData(["app-preferences"], defaultPreferences)
  })
  afterEach(async () => { await act(async () => root.unmount()); client.clear(); container.remove(); Reflect.deleteProperty(window, "__TAURI_INTERNALS__") })
  async function mount() { await act(async () => root.render(<QueryClientProvider client={client}><App /></QueryClientProvider>)) }
  async function settle() { await act(async () => { await new Promise((resolve) => setTimeout(resolve, 0)) }) }
  async function unlock(id = wallets[1].id) {
    await act(async () => { const select = container.querySelector('section[aria-label="Your saved wallets"] select') as HTMLSelectElement; select.value = id; select.dispatchEvent(new Event("change", { bubbles: true })) })
    container.querySelector<HTMLInputElement>('input[name="password"]')!.value = "short"
    await act(async () => container.querySelector('section[aria-label="Your saved wallets"] form')!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })))
    await settle()
  }

  it("opens the chosen wallet from Home on startup without storage, node or import steps", async () => {
    await mount()
    expect(container.querySelector('section[aria-label="Your saved wallets"] select')!.children).toHaveLength(3)
    await unlock()
    expect(openWallet).toHaveBeenCalledWith(wallets[1].id, "short")
    expect(container.querySelector("main")!.textContent).toContain("Saved wallet dashboard")
    expect(container.querySelector('input[name="seed"]')).toBeNull()
    for (const mutation of [createWallet, restoreWallet, importWallet]) expect(mutation).not.toHaveBeenCalled()
  })

  it("returns to the Home selector after inactivity lock and permits opening either saved wallet", async () => {
    await mount(); await unlock()
    state.lifecycle = { state: "locked", session_generation: "9" }
    await act(async () => state.listeners.get("wallet-auto-locked")!({ payload: state.lifecycle }))
    await settle()
    expect(container.querySelector("main")!.textContent).not.toContain("Saved wallet dashboard")
    expect(container.querySelector('section[aria-label="Your saved wallets"]')).not.toBeNull()
    expect(container.querySelector('input[type="password"]')).toBeNull()
    await unlock(wallets[0].id)
    expect(openWallet).toHaveBeenLastCalledWith(wallets[0].id, "short")
    expect(container.querySelector("main")!.textContent).toContain("Saved wallet dashboard")
  })

  it("returns to Home after manual locking as well", async () => {
    await mount(); await unlock()
    const lock = Array.from(container.querySelectorAll("button")).find((button) => button.textContent === "Lock")!
    await act(async () => lock.click())
    await settle()
    expect(lockWallet).toHaveBeenCalledOnce()
    expect(container.querySelector('section[aria-label="Your saved wallets"]')).not.toBeNull()
  })

  it("retains the existing backup check for saved wallets whose backup is incomplete", async () => {
    client.setQueryData(["wallet-list", "test-root", "mainnet"], [{ ...wallets[0], backup_complete: false }])
    vi.mocked(getBackupPhrase).mockResolvedValue({ recovery_phrase: "disposable test phrase" })
    await mount(); await unlock(wallets[0].id)
    expect(getBackupPhrase).toHaveBeenCalledWith(wallets[0].id)
    expect(container.querySelector("main")!.textContent).toContain("Back up your recovery phrase")
    expect(container.querySelector("main")!.textContent).not.toContain("Saved wallet dashboard")
  })

  it("discards the selection and password when the data folder and network change", async () => {
    await mount()
    await act(async () => { const select = container.querySelector("select")!; select.value = wallets[0].id; select.dispatchEvent(new Event("change", { bubbles: true })) })
    container.querySelector<HTMLInputElement>('input[name="password"]')!.value = "discard-me"
    const other = { id: "c".repeat(32), backup_complete: true }
    client.setQueryData(["wallet-list", "other-root", "testnet"], [other])
    await act(async () => client.setQueryData(["data-root-configured"], { root: "other-root", network: "testnet" }))
    await settle()
    const select = container.querySelector("select")!
    expect(select.value).toBe("")
    expect(Array.from(select.options).map((option) => option.value)).toEqual(["", other.id])
    expect(container.querySelector('input[type="password"]')).toBeNull()
    expect(openWallet).not.toHaveBeenCalled()
  })
})
