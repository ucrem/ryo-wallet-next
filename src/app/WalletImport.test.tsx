// @vitest-environment jsdom
import { act } from "react"
import { createRoot, type Root } from "react-dom/client"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { WalletWorkspace } from "./WalletWorkspace"
import { getBackupPhrase, importWallet } from "@/api/wallet"

vi.mock("@/api/wallet", () => ({
  listWallets: async () => [], walletRuntimeReady: async () => true,
  importWallet: vi.fn(), getBackupPhrase: vi.fn(), acknowledgeBackup: vi.fn(),
  createWallet: vi.fn(), restoreWallet: vi.fn(), openWallet: vi.fn(), lockWallet: vi.fn(),
}))
vi.mock("./WalletDashboard", () => ({ WalletDashboard: () => <article>Imported wallet dashboard</article> }))

describe("native wallet file import", () => {
  let container: HTMLDivElement
  let root: Root
  let client: QueryClient
  beforeEach(async () => {
    vi.clearAllMocks()
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
    container = document.createElement("div")
    document.body.append(container)
    root = createRoot(container)
    client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity, staleTime: Infinity } } })
    client.setQueryData(["wallet-list"], [])
    client.setQueryData(["wallet-runtime-ready"], true)
    await act(async () => root.render(
      <QueryClientProvider client={client}>
        <WalletWorkspace mode="open" activeWallet={null} sessionGeneration="7" onBack={() => {}} onLocked={() => {}} />
      </QueryClientProvider>,
    ))
  })
  afterEach(async () => {
    await act(async () => root.unmount())
    client.clear()
    container.remove()
  })
  function form() { return container.querySelector<HTMLFormElement>('section[aria-label="Import wallet file"] form')! }
  async function submit(backup = true) {
    form().querySelector<HTMLInputElement>('input[name="password"]')!.value = "short"
    form().querySelector<HTMLInputElement>('input[name="backup"]')!.checked = backup
    await act(async () => { form().dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })) })
  }

  it("offers import with an empty wallet list and accepts existing short passwords", async () => {
    expect(container.textContent).toContain("No saved wallets")
    expect(form().querySelector<HTMLInputElement>('input[name="password"]')!.minLength).toBe(1)
    await submit(false)
    expect(importWallet).not.toHaveBeenCalled()
    vi.mocked(importWallet).mockResolvedValue({ wallet_id: "imported", status: { state: "open", session_generation: "7" } })
    await submit()
    expect(importWallet).toHaveBeenCalledWith("short", true)
    expect(container.textContent).toContain("Imported wallet dashboard")
    expect(getBackupPhrase).not.toHaveBeenCalled()
  })
  it("treats cancelled native selection as cancellation and clears the password", async () => {
    vi.mocked(importWallet).mockResolvedValue(null)
    await submit()
    expect(container.textContent).toContain("File selection cancelled. No wallet was imported.")
    expect(form().querySelector<HTMLInputElement>('input[name="password"]')!.value).toBe("")
    expect(container.textContent).not.toContain("Imported wallet dashboard")
    expect(getBackupPhrase).not.toHaveBeenCalled()
  })
  it("keeps import available after an incorrect password without showing a recovery phrase", async () => {
    vi.mocked(importWallet).mockRejectedValue("incorrect wallet password")
    await submit()
    expect(container.querySelector('[role="alert"]')?.textContent).toBe("incorrect wallet password")
    expect(form().querySelector<HTMLInputElement>('input[name="password"]')!.value).toBe("")
    expect(form().querySelector<HTMLButtonElement>('button[type="submit"]')!.disabled).toBe(false)
    expect(getBackupPhrase).not.toHaveBeenCalled()
  })
})
