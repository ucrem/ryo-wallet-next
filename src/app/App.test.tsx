import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { renderToStaticMarkup } from "react-dom/server"
import { afterEach, describe, expect, it, vi } from "vitest"
import { App, PageHeader, resolveVisibleScreen, type Screen } from "./App"

afterEach(() => vi.unstubAllGlobals())

describe("compact desktop page headings", () => {
  it.each([
    ["home", "Start"], ["storage", "Data location"], ["node", "Node"],
    ["summary", "Summary"], ["wallet", "Wallet"], ["activity", "Activity"], ["about", "About"],
  ] as [Screen, string][])("shows the %s title in the application top bar", (screen, title) => {
    const markup = renderToStaticMarkup(<PageHeader screen={screen} />)
    expect(markup).toContain(`<h1 id="page-title"`)
    expect(markup).toContain(`>${title}</h1>`)
    expect(markup).toContain("Mainnet · Preview")
  })

  it("starts with one page heading and keeps Activity inaccessible without a wallet", () => {
    vi.stubGlobal("window", {})
    const client = new QueryClient()
    const markup = renderToStaticMarkup(<QueryClientProvider client={client}><App /></QueryClientProvider>)
    expect(markup.match(/<h1\b/g)).toHaveLength(1)
    expect(markup).toContain("aria-labelledby=\"page-title\"")
    expect(markup).not.toContain("GET STARTED")
    expect(markup).not.toContain("How would you like to use Ryo?")
    expect(markup).toMatch(/disabled=""[^>]*>[^<]*<span[^>]*>05<\/span>Activity<\/button>/)
  })

  it("leaves Activity immediately when the wallet locks or its session changes", () => {
    expect(resolveVisibleScreen("activity", true, "2", "2")).toBe("activity")
    expect(resolveVisibleScreen("activity", false, "2", "2")).toBe("wallet")
    expect(resolveVisibleScreen("activity", true, "2", "3")).toBe("wallet")
  })
})
