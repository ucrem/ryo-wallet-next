import { readFile } from "node:fs/promises"
import { URL } from "node:url"

const version = process.argv[2]
if (!version || !/^[0-9A-Za-z.+-]+$/.test(version)) throw new Error("A release version is required")
const changelog = await readFile(new URL("../CHANGELOG.md", import.meta.url), "utf8")
const lines = changelog.split(/\r?\n/)
const start = lines.findIndex((line) => line.startsWith(`## [${version}] - `))
if (start < 0) throw new Error(`Missing changelog entry for ${version}`)
const next = lines.findIndex((line, index) => index > start && line.startsWith("## "))
process.stdout.write(`# Ryo Wallet Next v${version}\n\n${lines.slice(start + 1, next < 0 ? undefined : next).join("\n").trim()}\n\n`)
