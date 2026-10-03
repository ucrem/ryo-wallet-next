import { renderToStaticMarkup } from "react-dom/server"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { describe, expect, it } from "vitest"
import { WalletWorkspace } from "./WalletWorkspace"

describe("wallet restoration entry", () => {
  it("offers phrase, password and scan-height inputs with embedded-height guidance", () => {
    const markup = renderToStaticMarkup(
      <QueryClientProvider client={new QueryClient()}>
        <WalletWorkspace mode="restore" activeWallet={null} sessionGeneration={null}
          onBack={() => {}} onLocked={() => {}} />
      </QueryClientProvider>,
    )

    expect(markup).toContain("Restore wallet")
    expect(markup).toContain('name="seed"')
    expect(markup).toContain('name="password"')
    expect(markup).toContain('name="height"')
    expect(markup).toContain("height embedded in the phrase")
    expect(markup).not.toContain("No app-owned wallets were found")
  })
})
