import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, it } from "vitest"
import { SendReview } from "./WalletDashboard"
import { exactAmount, formatRyo } from "@/lib/walletFormatting"
import type { SendDraft } from "@/api/operations"

describe("transaction review", () => {
  it("shows the immutable full address, exact amount and actual fee and disables unapproved relay", () => {
    const draft: SendDraft = { token: "opaque", address: "RYoL-full-recipient-address", payment_id: "a".repeat(64), amount: "9007199254740993",
      fee: "201", total: "9007199254741194", transactions: ["b".repeat(64)], expires_in_seconds: 300 }
    const html = renderToStaticMarkup(<SendReview draft={draft} busy={false} onCancel={() => {}} onConfirm={() => {}} />)
    expect(html).toContain(draft.address)
    expect(html).toContain("9007199.254740993 RYO")
    expect(html).toContain("0.000000201 RYO")
    expect(html).toContain("9007199.254741194 RYO")
    expect(html).toContain('disabled=""')
    expect(html).toContain("I checked the full address, amount and fee")
  })
  it("blocks expired previews", () => {
    const html = renderToStaticMarkup(<SendReview draft={{ token: "expired", address: "recipient", payment_id: "", amount: "1", fee: "1", total: "2", transactions: [], expires_in_seconds: 0 }} busy={false} onCancel={() => {}} onConfirm={() => {}} />)
    expect(html).toContain("Preview expired")
    expect(html).toContain('disabled=""')
  })
  it("preserves atomic precision and rejects rounded or unsupported amounts", () => {
    expect(formatRyo("18446744073709551615")).toBe("18446744073.709551615")
    expect(exactAmount("18446744073.709551615")).toBe(true)
    for (const value of ["18446744073.709551616", "1.0000000001", "1e3", "1,2", "0", "-1"]) expect(exactAmount(value)).toBe(false)
  })
})
