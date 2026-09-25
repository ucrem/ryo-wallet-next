import { describe, expect, it } from "vitest"
import { displayVersion, releaseStatus, releaseUrl } from "./appVersion"

describe("version presentation", () => {
  it("derives preview status from the version", () => {
    expect(releaseStatus("0.1.0-alpha.2")).toBe("Alpha preview")
    expect(releaseStatus("0.1.0-beta.1")).toBe("Beta preview")
    expect(releaseStatus("0.1.0-rc.1")).toBe("Release candidate")
    expect(releaseStatus("1.0.0")).toBe("Release")
  })

  it("formats the version and restricts release links to version-shaped tags", () => {
    expect(displayVersion("0.1.0-alpha.2")).toBe("v0.1.0-alpha.2")
    expect(releaseUrl("0.1.0-alpha.2")).toBe("https://github.com/ucrem/ryo-wallet-next/releases/tag/v0.1.0-alpha.2")
    expect(releaseUrl("https://example.com/")).toBeNull()
  })
})
