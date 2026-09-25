import console from "node:console"
import { readFileSync } from "node:fs"
import { dirname, resolve } from "node:path"
import process from "node:process"
import { fileURLToPath } from "node:url"

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..")

function read(path) {
  try {
    return readFileSync(resolve(root, path), "utf8")
  } catch {
    return null
  }
}

function jsonVersion(path) {
  const text = read(path)
  if (!text) return null
  try {
    return JSON.parse(text).version ?? null
  } catch {
    return null
  }
}

function tomlPackageVersion(text) {
  if (!text) return null
  const lines = text.split(/\r?\n/)
  const start = lines.findIndex((line) => line.trim() === "[package]")
  if (start < 0) return null
  for (const line of lines.slice(start + 1)) {
    if (line.trim().startsWith("[")) break
    const match = /^version\s*=\s*"([^"]+)"\s*(?:#.*)?$/.exec(line.trim())
    if (match) return match[1]
  }
  return null
}

function lockPackageVersion(text) {
  if (!text) return null
  for (const block of text.split(/^\[\[package\]\]\s*$/m).slice(1)) {
    if (!/^name\s*=\s*"ryo-wallet-next"\s*$/m.test(block)) continue
    return /^version\s*=\s*"([^"]+)"\s*$/m.exec(block)?.[1] ?? null
  }
  return null
}

function newestChangelogVersion(text) {
  if (!text) return null
  const heading = text.split(/\r?\n/).map((line) => line.trim()).find((line) => line.startsWith("##"))
  if (!heading) return null
  const match = /^## \[([0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?)\] - (\d{4}-\d{2}-\d{2})$/.exec(heading)
  if (!match) return null
  const date = new Date(`${match[2]}T00:00:00Z`)
  if (Number.isNaN(date.getTime()) || date.toISOString().slice(0, 10) !== match[2]) return null
  return match[1]
}

const versions = {
  "package.json": jsonVersion("package.json"),
  "src-tauri/tauri.conf.json": jsonVersion("src-tauri/tauri.conf.json"),
  "src-tauri/Cargo.toml": tomlPackageVersion(read("src-tauri/Cargo.toml")),
  "Cargo.lock (ryo-wallet-next)": lockPackageVersion(read("Cargo.lock")),
  "CHANGELOG.md (newest release)": newestChangelogVersion(read("CHANGELOG.md")),
}

const values = Object.values(versions)
const valid = values.every((value) => typeof value === "string" && value.length > 0)
  && values.every((value) => value === values[0])

if (!valid) {
  console.error("Application versions disagree or a version could not be parsed:")
  for (const [source, version] of Object.entries(versions)) {
    console.error(`  ${source}: ${version ?? "<missing or invalid>"}`)
  }
  process.exitCode = 1
} else {
  console.log(`Version consistency OK: ${values[0]}`)
}
