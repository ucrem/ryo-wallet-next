import { describe, expect, it } from "vitest"
import { parseChangelog } from "./changelog"

const sample = `# Changelog

## [0.1.0-alpha.2] - 2026-09-25

### Added
- First item
- Second item

### Changed
- Third item

## [0.1.0-alpha.1] - 2026-09-24

### Added
- Earlier item
`

describe("parseChangelog", () => {
  it("preserves prerelease, section, bullet, and newest-first release ordering", () => {
    expect(parseChangelog(sample)).toEqual([
      {
        version: "0.1.0-alpha.2",
        date: "2026-09-25",
        sections: [
          { title: "Added", items: ["First item", "Second item"] },
          { title: "Changed", items: ["Third item"] },
        ],
      },
      {
        version: "0.1.0-alpha.1",
        date: "2026-09-24",
        sections: [{ title: "Added", items: ["Earlier item"] }],
      },
    ])
  })

  it("ignores blank lines without changing release order", () => {
    expect(parseChangelog(`\n${sample}\n\n`).map((release) => release.version)).toEqual([
      "0.1.0-alpha.2",
      "0.1.0-alpha.1",
    ])
  })

  it.each([
    "## 0.1.0-alpha.2 - 2026-09-25",
    "## [0.1.0-alpha.2] 2026-09-25",
    "## [0.1.0-alpha.2] - 2026-02-30",
  ])("rejects malformed release headings: %s", (heading) => {
    expect(() => parseChangelog(`# Changelog\n${heading}\n`)).toThrow(/release heading at line 2/)
  })

  it("rejects items without a release and section", () => {
    expect(() => parseChangelog("# Changelog\n- orphan")).toThrow(/line 2/)
  })

  it("keeps HTML-like text as plain item text", () => {
    expect(parseChangelog("## [1.0.0] - 2026-09-25\n### Added\n- <script>alert(1)</script>")[0].sections[0].items)
      .toEqual(["<script>alert(1)</script>"])
  })
})
