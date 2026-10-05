// @vitest-environment jsdom
import { act } from "react"
import { createRoot } from "react-dom/client"
import { afterEach, describe, expect, it, vi } from "vitest"
import { invoke } from "@tauri-apps/api/core"
import { Diagnostics } from "./Diagnostics"

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
const container = document.createElement("div")
document.body.append(container)
let root = createRoot(container)
afterEach(async () => {
  await act(async () => root.unmount())
  root = createRoot(container)
  vi.clearAllMocks()
})

describe("diagnostic export", () => {
  it("allows native cancellation and excludes raw error text from feedback", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(false)
    await act(async () => root.render(<Diagnostics isNative />))
    await act(async () => container.querySelector("button")!.click())
    expect(invoke).toHaveBeenCalledWith("app_export_diagnostics")
    expect(container.textContent).toContain("Export cancelled.")
    vi.mocked(invoke).mockRejectedValueOnce("password-path-canary")
    await act(async () => container.querySelector("button")!.click())
    expect(container.querySelector('[role="alert"]')?.textContent).toContain("Choose a new JSON filename")
    expect(container.textContent).not.toContain("password-path-canary")
  })

  it("does not dispatch from a browser preview and dispatches only once while saving", async () => {
    await act(async () => root.render(<Diagnostics isNative={false} />))
    expect(container.querySelector("button")!.disabled).toBe(true)
    container.querySelector("button")!.click()
    expect(invoke).not.toHaveBeenCalled()
    let finish: (saved: boolean) => void = () => {}
    vi.mocked(invoke).mockImplementationOnce(() => new Promise((resolve) => { finish = resolve as (saved: boolean) => void }))
    await act(async () => root.render(<Diagnostics isNative />))
    await act(async () => {
      container.querySelector("button")!.click()
      container.querySelector("button")!.click()
    })
    expect(invoke).toHaveBeenCalledTimes(1)
    expect(container.querySelector("button")!.disabled).toBe(true)
    await act(async () => finish(true))
    expect(container.textContent).toContain("Nothing was uploaded.")
  })
})
