export type ChangelogRelease = {
  version: string
  date: string
  sections: { title: string; items: string[] }[]
}

const releaseHeading = /^## \[([0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?)\] - (\d{4}-\d{2}-\d{2})$/

export function parseChangelog(markdown: string): ChangelogRelease[] {
  const releases: ChangelogRelease[] = []
  let currentRelease: ChangelogRelease | null = null
  let currentSection: ChangelogRelease["sections"][number] | null = null

  for (const [index, rawLine] of markdown.split(/\r?\n/).entries()) {
    const line = rawLine.trim()
    if (!line || line === "# Changelog") continue

    if (line.startsWith("###")) {
      const title = line.startsWith("### ") ? line.slice(4).trim() : ""
      if (!currentRelease || !title) {
        throw new Error(`Invalid changelog section at line ${index + 1}: ${line}`)
      }
      currentSection = { title, items: [] }
      currentRelease.sections.push(currentSection)
      continue
    }

    if (line.startsWith("##")) {
      const match = releaseHeading.exec(line)
      if (!match || !isValidDate(match[2])) {
        throw new Error(`Invalid changelog release heading at line ${index + 1}: ${line}`)
      }
      currentRelease = { version: match[1], date: match[2], sections: [] }
      releases.push(currentRelease)
      currentSection = null
      continue
    }

    if (line.startsWith("- ") && currentSection) {
      const item = line.slice(2).trim()
      if (!item) throw new Error(`Empty changelog item at line ${index + 1}`)
      currentSection.items.push(item)
      continue
    }

    throw new Error(`Unexpected changelog content at line ${index + 1}: ${line}`)
  }

  if (!releases.length) throw new Error("Changelog has no release entries")
  return releases
}

function isValidDate(value: string): boolean {
  const date = new Date(`${value}T00:00:00Z`)
  return !Number.isNaN(date.getTime()) && date.toISOString().slice(0, 10) === value
}
