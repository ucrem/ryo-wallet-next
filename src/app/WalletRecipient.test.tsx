// @vitest-environment jsdom
// Regression coverage for the reported recipient and copy-state defects.
import { act } from "react"
import { createRoot, type Root } from "react-dom/client"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { afterEach, beforeEach, expect, it, vi } from "vitest"
import { WalletDashboard, SendReview } from "@/app/WalletDashboard"
import { Settings } from "@/app/Settings"
import { defaultPreferences } from "@/api/settings"
import type { Contact, WalletSection } from "@/api/operations"

const api = vi.hoisted(() => ({ writeText: vi.fn(), walletOperation: vi.fn(), getWalletOverview: vi.fn(), getReceiveAddresses: vi.fn(), getWalletSyncStatus: vi.fn(), savePreferences: vi.fn() }))
vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({ writeText: api.writeText }))
vi.mock("@/api/overview", () => ({ getWalletOverview: api.getWalletOverview }))
vi.mock("@/api/wallet", () => ({ getReceiveAddresses: api.getReceiveAddresses, getWalletSyncStatus: api.getWalletSyncStatus, createReceiveAddress: vi.fn() }))
vi.mock("@/api/operations", () => ({ walletOperation: api.walletOperation, exportArtwork: vi.fn(), manageKeyImages: vi.fn(), removeWallet: vi.fn() }))
vi.mock("@/api/settings", async (original) => ({ ...await original<typeof import("@/api/settings")>(), savePreferences: api.savePreferences }))

let container: HTMLDivElement, root: Root, client: QueryClient, contacts: Contact[]
const firstAddress = "audit-primary-address-no-real-wallet"
const secondAddress = "audit-subaddress-no-real-wallet"
beforeEach(() => {
  vi.resetAllMocks()
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
  container = document.createElement("div"); document.body.append(container); root = createRoot(container)
  client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity } } })
  client.setQueryData(["app-preferences"], defaultPreferences)
  contacts = [{ id: "audit-contact", name: "Audit recipient", address: secondAddress, payment_id: "", notes: "" }]
  api.writeText.mockResolvedValue(undefined)
  api.getWalletOverview.mockResolvedValue({session_generation:"7",primary_address:firstAddress,total:{atomic:"10000000000"},unlocked:{atomic:"10000000000"},locked:{atomic:"0"},multisig_import_needed:false})
  api.getReceiveAddresses.mockResolvedValue([{address_index:0,address:firstAddress,label:"Primary",used:false},{address_index:1,address:secondAddress,label:"Second",used:false}])
  api.getWalletSyncStatus.mockResolvedValue({wallet_height:"100",daemon_height:"100",network_height:"100",node_reachable:true,node_ready:true,node_offline:false,node_untrusted:false,wallet_rpc_busy:false,wallet_rpc_available:true})
  api.walletOperation.mockImplementation(async (_generation, operation) => {
    if (operation.type === "info") return {name:"Audit wallet"}
    if (operation.type === "contacts") return [...contacts]
    if (operation.type === "delete_contact") { contacts = contacts.filter(entry => entry.id !== operation.id); return {} }
    if (operation.type === "address_balances") return [{index:0,balance:"0",unlocked:"0",outputs:"0"},{index:1,balance:"0",unlocked:"0",outputs:"0"}]
    if (operation.type === "make_request") return {uri:`ryo:${operation.address}?tx_amount=1`}
    return []
  })
  api.savePreferences.mockImplementation(async preferences => preferences)
  vi.spyOn(window,"confirm").mockReturnValue(true)
})
afterEach(async () => { await act(async () => root.unmount()); client.clear(); container.remove(); vi.restoreAllMocks() })
const button = (label: string) => Array.from(container.querySelectorAll<HTMLButtonElement>("button")).find(entry => entry.textContent === label)!
async function render(element: React.ReactNode) {
  await act(async () => { root.render(<QueryClientProvider client={client}>{element}</QueryClientProvider>); await new Promise(resolve => setTimeout(resolve,30)) })
  await act(async () => { await new Promise(resolve => setTimeout(resolve,30)) })
}
async function wallet(section: WalletSection) { await render(<WalletDashboard generation="7" section={section} onSection={() => {}} onLock={() => {}} onRemoved={() => {}} locking={false} />) }

it("receive copy acknowledgement must clear after selecting a different address", async () => {
  await wallet("receive")
  await act(async () => button("Copy address").click())
  expect(api.writeText).toHaveBeenLastCalledWith(firstAddress)
  expect(button("Copied")).toBeDefined()
  const second = Array.from(container.querySelectorAll<HTMLButtonElement>('[aria-label="Receive address selection"] button')).find(entry => entry.textContent!.includes("Second"))!
  await act(async () => second.click())
  expect(container.textContent).toContain(secondAddress)
  expect(api.writeText).toHaveBeenCalledTimes(1)
  expect(button("Copy address"),"The newly selected address has not been copied yet").toBeDefined()
})

it("deleting a contact must clear its retained Send recipient", async () => {
  await wallet("contacts")
  await act(async () => button("Send").click())
  await wallet("send")
  expect(container.querySelector<HTMLInputElement>('input[maxlength="256"]')!.value).toBe(secondAddress)
  await wallet("contacts")
  await act(async () => { button("Delete").click(); await new Promise(resolve => setTimeout(resolve,30)) })
  expect(contacts).toHaveLength(0)
  await wallet("send")
  expect(container.querySelector<HTMLInputElement>('input[maxlength="256"]')!.value,"An explicitly deleted contact must not silently remain the default recipient").toBe("")
})

it("choosing a contact and then editing its address must clear the contact selector", async () => {
  await wallet("send")
  const select = Array.from(container.querySelectorAll<HTMLSelectElement>("select")).find(entry => Array.from(entry.options).some(option => option.value === "audit-contact"))!
  await act(async () => { select.value="audit-contact"; select.dispatchEvent(new Event("change",{bubbles:true})) })
  const address = container.querySelector<HTMLInputElement>('input[maxlength="256"]')!
  expect(address.value).toBe(secondAddress)
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,"value")!.set!
  await act(async () => { setter.call(address,firstAddress); address.dispatchEvent(new Event("input",{bubbles:true})) })
  expect(address.value).toBe(firstAddress)
  expect(select.value,"Address book selection should describe the visible recipient").toBe("")
})

it("clearing a failed clipboard acknowledgement permits retry", async () => {
  api.writeText.mockRejectedValueOnce(new Error("audit clipboard denial"))
  await wallet("receive")
  await act(async () => button("Copy address").click())
  expect(button("Copy failed")).toBeDefined()
  await act(async () => button("Copy failed").click())
  expect(button("Copied")).toBeDefined()
  expect(api.writeText).toHaveBeenCalledTimes(2)
})

it("a late clipboard response cannot acknowledge the newly selected address", async () => {
  let complete!: () => void
  api.writeText.mockImplementationOnce(() => new Promise<void>((resolve) => { complete = resolve }))
  await wallet("receive")
  await act(async () => button("Copy address").click())
  const second = Array.from(container.querySelectorAll<HTMLButtonElement>('[aria-label="Receive address selection"] button')).find(entry => entry.textContent!.includes("Second"))!
  await act(async () => second.click())
  await act(async () => complete())
  expect(button("Copied")).toBeUndefined()
  expect(button("Copy address")).toBeDefined()
  await act(async () => button("Copy address").click())
  expect(api.writeText).toHaveBeenLastCalledWith(secondAddress)
  expect(button("Copied")).toBeDefined()
})

it("an address-book navigation intent for a deleted contact is ignored", async () => {
  client.setQueryData(["send-contact", "7"], contacts[0])
  contacts = []
  await wallet("send")
  expect(container.querySelector<HTMLInputElement>('input[maxlength="256"]')!.value).toBe("")
})

it("editing a contact payment ID clears selection and permits choosing it again", async () => {
  contacts[0].payment_id = "1234567890123456"
  await wallet("send")
  const select = Array.from(container.querySelectorAll<HTMLSelectElement>("select")).find(entry => Array.from(entry.options).some(option => option.value === "audit-contact"))!
  await act(async () => { select.value = "audit-contact"; select.dispatchEvent(new Event("change", { bubbles: true })) })
  const payment = container.querySelector<HTMLInputElement>('input[maxlength="64"]')!
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!
  await act(async () => { setter.call(payment, "another-payment-id"); payment.dispatchEvent(new Event("input", { bubbles: true })) })
  expect(select.value).toBe("")
  await act(async () => { select.value = "audit-contact"; select.dispatchEvent(new Event("change", { bubbles: true })) })
  expect(payment.value).toBe("1234567890123456")
  expect(select.value).toBe("audit-contact")
})

it("an expired send review cannot confirm", async () => {
  const confirm = vi.fn()
  await render(<SendReview draft={{token:"audit-token",address:secondAddress,payment_id:"",amount:"1",fee:"1",total:"2",transactions:[],expires_in_seconds:0}} busy={false} warnNoPaymentId={false} onConfirm={confirm} onCancel={() => {}} />)
  const acknowledgement = container.querySelector<HTMLInputElement>('input[type="checkbox"]')!
  expect(acknowledgement.disabled).toBe(true)
  expect(button("Confirm and send").disabled).toBe(true)
  expect(confirm).not.toHaveBeenCalled()
})

it("preferences save while General settings are locked", async () => {
  await render(<Settings root="audit-root" node={{mode:"local",network:"mainnet",host:"127.0.0.1",port:12211,trust:"managed_local"}} network="mainnet" busy />)
  expect(button("Save").disabled).toBe(true)
  await act(async () => button("Preferences").click())
  await act(async () => button("Save").click())
  expect(api.savePreferences).toHaveBeenCalledOnce()
})
