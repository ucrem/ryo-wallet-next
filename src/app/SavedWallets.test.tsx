// @vitest-environment jsdom
import { act } from "react"
import { createRoot, type Root } from "react-dom/client"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { SavedWallets } from "./SavedWallets"
import { listWallets, openWallet } from "@/api/wallet"

vi.mock("@/api/wallet", () => ({ listWallets: vi.fn(), openWallet: vi.fn(), walletRuntimeReady: async () => true }))

const wallets = [{ id: "a".repeat(32), backup_complete: true }, { id: "b".repeat(32), backup_complete: true }]
describe("saved wallet unlocking", () => {
  let container: HTMLDivElement, root: Root, client: QueryClient
  const opened = vi.fn()
  beforeEach(() => {
    vi.resetAllMocks()
    vi.mocked(listWallets).mockResolvedValue(wallets)
    vi.mocked(openWallet).mockResolvedValue({ state: "open", session_generation: "8" })
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
    container = document.createElement("div"); document.body.append(container); root = createRoot(container)
    client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity, staleTime: Infinity } } })
    client.setQueryData(["wallet-list", "test-root", "mainnet"], wallets)
    client.setQueryData(["wallet-runtime-ready"], true)
  })
  afterEach(async () => { await act(async () => root.unmount()); client.clear(); container.remove() })
  async function show(disabled = false) {
    await act(async () => root.render(<QueryClientProvider client={client}><SavedWallets root="test-root" network="mainnet" disabled={disabled} nodeConfigured onOpened={opened} onConfigure={() => {}} /></QueryClientProvider>))
  }
  async function select(id: string) {
    await act(async () => { const select = container.querySelector("select")!; select.value = id; select.dispatchEvent(new Event("change", { bubbles: true })) })
  }
  function submit() { container.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })) }

  it("clears the password on a wallet change and permits retry after an incorrect password", async () => {
    await show()
    expect(container.querySelector('input[type="password"]')).toBeNull()
    await select(wallets[0].id)
    container.querySelector<HTMLInputElement>('input[name="password"]')!.value = "first-password"
    await select(wallets[1].id)
    expect(container.querySelector<HTMLInputElement>('input[name="password"]')!.value).toBe("")
    vi.mocked(openWallet).mockRejectedValueOnce("incorrect password")
    container.querySelector<HTMLInputElement>('input[name="password"]')!.value = "wrong-password"
    await act(async () => submit())
    expect(container.querySelector('[role="alert"]')!.textContent).toBe("incorrect password")
    expect(container.querySelector<HTMLInputElement>('input[name="password"]')!.value).toBe("")
    expect(container.querySelector("select")!.value).toBe(wallets[1].id)
    expect(opened).not.toHaveBeenCalled()
    container.querySelector<HTMLInputElement>('input[name="password"]')!.value = "short"
    await act(async () => submit())
    expect(openWallet).toHaveBeenLastCalledWith(wallets[1].id, "short")
    expect(opened).toHaveBeenCalledOnce()
    expect(client.getQueryData(["foundation-status"])).toEqual({ state: "open", session_generation: "8" })
  })

  it("submits only once while unlocking and ignores submissions when the session is unavailable", async () => {
    await show()
    await select(wallets[0].id)
    let finish!: (value: { state: "open"; session_generation: string }) => void
    vi.mocked(openWallet).mockReturnValueOnce(new Promise((resolve) => { finish = resolve }))
    container.querySelector<HTMLInputElement>('input[name="password"]')!.value = "short"
    await act(async () => { submit(); submit() })
    expect(openWallet).toHaveBeenCalledOnce()
    expect(container.querySelector("select")!.matches(":disabled")).toBe(true)
    await act(async () => finish({ state: "open", session_generation: "8" }))
    await show(true)
    container.querySelector<HTMLInputElement>('input[name="password"]')!.value = "another"
    await act(async () => submit())
    expect(openWallet).toHaveBeenCalledOnce()
  })

  it("offers retry for unreadable wallet lists without presenting stale unlock controls", async () => {
    client.removeQueries({ queryKey: ["wallet-list"] })
    vi.mocked(listWallets).mockRejectedValue("list unavailable")
    await show()
    await act(async () => { await new Promise((resolve) => setTimeout(resolve, 0)) })
    expect(container.textContent).toContain("Saved wallets could not be read")
    expect(container.querySelector("form")).toBeNull()
    vi.mocked(listWallets).mockResolvedValue(wallets)
    await act(async () => container.querySelector<HTMLButtonElement>("button")!.click())
    await act(async () => { await new Promise((resolve) => setTimeout(resolve, 0)) })
    expect(container.querySelector("select")!.options).toHaveLength(3)
  })
})
