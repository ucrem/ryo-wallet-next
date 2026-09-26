import { describe, expect, it } from "vitest"
import { renderToStaticMarkup } from "react-dom/server"
import { About } from "./About"
import type { AppUpdates } from "@/lib/useAppUpdates"
import type { AppVersionInfo } from "@/lib/useAppVersion"

const version: AppVersionInfo = {
  version: "0.1.0-alpha.4",
  label: "v0.1.0-alpha.4",
  status: "Alpha preview",
  isNative: true,
  isLoading: false,
  isError: false,
}

function html(overrides: Partial<AppUpdates>): string {
  const updates: AppUpdates = {
    result: null,
    checking: false,
    installing: false,
    checked: false,
    error: null,
    progress: null,
    checkNow: async () => {},
    install: async () => {},
    ...overrides,
  }
  return renderToStaticMarkup(<About appVersion={version} updates={updates} />)
}

describe("installed update feedback", () => {
  it("disables update checks in development", () => {
    const markup = html({ result: { state: "development" } })
    expect(markup).toContain("Unavailable in development")
    expect(markup).toMatch(/disabled=""[^>]*>Unavailable in development/)
  })

  it("shows a live checking region", () => {
    const markup = html({ checking: true })
    expect(markup).toContain("Checking for updates…")
    expect(markup).toContain('aria-live="polite"')
    expect(markup).toContain('role="status"')
  })

  it("confirms the installed version when already current", () => {
    expect(html({ result: { state: "current" } })).toContain(
      "Ryo Wallet Next is already up to date (v0.1.0-alpha.4).",
    )
  })

  it("keeps install and package-manager update paths visible", () => {
    expect(html({ result: { state: "available", version: "0.1.0-alpha.5", notes: null, automatic_install: true } }))
      .toContain("Download and install")
    expect(html({ result: { state: "available", version: "0.1.0-alpha.5", notes: null, automatic_install: false } }))
      .toContain("Open release page")
    expect(html({
      result: { state: "available", version: "0.1.0-alpha.5", notes: null, automatic_install: true },
      installing: true,
      progress: { downloaded: 2_000_000, total: 4_000_000 },
    })).toContain("Downloaded 2.0 MB of 4.0 MB")
  })

  it("shows retryable check errors", () => {
    const markup = html({ error: "Could not check for updates. Check your connection and try again." })
    expect(markup).toContain('role="alert"')
    expect(markup).toContain("Could not check for updates.")
    expect(markup).toContain("Check for updates")
  })
})
