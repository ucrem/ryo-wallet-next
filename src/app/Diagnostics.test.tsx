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
    expect(container.querySelector('[role="alert"]')?.textContent).toBe("Could not save the report. Please try again.")
    expect(container.textContent).not.toContain("password-path-canary")
  })

  it("distinguishes an existing file from protected storage and redacts raw error details", async () => {
    await act(async () => root.render(<Diagnostics isNative />))
    vi.mocked(invoke).mockRejectedValueOnce({ code: "file_exists", os_error_code: null, message: "private-path-canary" })
    await act(async () => container.querySelector("button")!.click())
    expect(container.querySelector('[role="alert"]')?.textContent).toBe("A file with this name already exists. Choose a different filename.")
    expect(container.textContent).not.toContain("private-path-canary")
    expect(container.textContent).not.toContain("private data folders")

    vi.mocked(invoke).mockRejectedValueOnce({ code: "protected_storage", os_error_code: null })
    await act(async () => container.querySelector("button")!.click())
    expect(container.querySelector('[role="alert"]')?.textContent).toContain("outside the application's private data folders")

    vi.mocked(invoke).mockRejectedValueOnce({ code: "permission_denied", os_error_code: 5, message: "private-path-canary" })
    await act(async () => container.querySelector("button")!.click())
    expect(container.querySelector('[role="alert"]')?.textContent).toContain("Choose a writable folder.")
    expect(container.querySelector('[role="alert"]')?.textContent).toContain("System error 5")
    expect(container.textContent).not.toContain("private-path-canary")

    vi.mocked(invoke).mockRejectedValueOnce({ code: "private-code-canary", os_error_code: "private-path-canary" })
    await act(async () => container.querySelector("button")!.click())
    expect(container.querySelector('[role="alert"]')?.textContent).toBe("Could not save the report. Please try again.")
    expect(container.textContent).not.toContain("canary")

    vi.mocked(invoke).mockResolvedValueOnce(true)
    await act(async () => container.querySelector("button")!.click())
    expect(container.querySelector('[role="alert"]')).toBeNull()
    expect(container.textContent).toContain("Diagnostic report saved.")
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
