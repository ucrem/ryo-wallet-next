import changelogText from "../../CHANGELOG.md?raw"
import { parseChangelog } from "@/lib/changelog"

export const bundledChangelog = parseChangelog(changelogText)
