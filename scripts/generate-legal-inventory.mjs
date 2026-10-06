// Regenerate after reviewing lockfile changes. This reads installed package
// metadata/notices; it never downloads or executes code linked from a notice.
import { spawnSync } from "node:child_process"
import { Buffer } from "node:buffer"
import { readFile, readdir, realpath, writeFile } from "node:fs/promises"
import { dirname, join, relative, sep } from "node:path"
import { projectRoot, legalRoot, digest } from "./legal-resources.mjs"

function jsonCommand(command, args) {
  // pnpm is a .cmd launcher on Windows. Keep its shell command literal; no
  // interpolated arguments or metadata ever become shell input.
  const windowsPnpm = process.platform === "win32" && command === "pnpm"
  const result = spawnSync(windowsPnpm ? "cmd.exe" : command, windowsPnpm ? ["/d", "/s", "/c", "pnpm licenses list --prod --json"] : args,
    { cwd: projectRoot, encoding: "utf8", maxBuffer: 32 * 1024 * 1024, windowsHide: true })
  if (result.error || result.status !== 0) throw new Error("Could not collect dependency license metadata")
  const start = result.stdout.indexOf("{")
  return JSON.parse(result.stdout.slice(start))
}

const cargo = jsonCommand("cargo", ["metadata", "--locked", "--format-version", "1"])
const npm = jsonCommand("pnpm", ["licenses", "list", "--prod", "--json"])
const notices = []
const supplemental = JSON.parse(await readFile(join(legalRoot, "supplemental-inventory.json"), "utf8"))
const usedSupplemental = new Set()
async function supplementaryNotices(ecosystem, name, version) {
  const key = `${ecosystem}:${name}@${version}`
  const record = supplemental.records.find(item => `${item.ecosystem}:${item.name}@${item.version}` === key)
  if (!record) return []
  usedSupplemental.add(key)
  const result = []
  for (const notice of record.notices) {
    if (!/^[0-9a-f]{64}\.txt$/.test(notice.file)) throw new Error("Invalid supplemental notice filename")
    const bytes = await readFile(join(legalRoot, "supplemental", notice.file))
    if (bytes.length > 262144 || bytes.includes(0) || digest(bytes) !== notice.sha256) throw new Error("Reviewed supplemental notice changed")
    notices.push(`===== ${ecosystem} ${name}@${version} / upstream ${notice.source_path} =====\n${bytes.toString("utf8").trim()}\n`)
    result.push({ file: `supplemental/${notice.file}`, sha256: notice.sha256, source: notice.source, source_commit: record.source_commit, publication_provenance: record.publication_provenance })
  }
  return result
}
async function licenseTexts(directory, label) {
  const root = await realpath(directory)
  const entries = (await readdir(root, { withFileTypes: true })).filter(entry => entry.isFile() && /^(license|licence|copying|copyright|notice|authors)([._-]|$)/i.test(entry.name)).sort((a, b) => a.name.localeCompare(b.name))
  const names = []
  for (const entry of entries) {
    const path = await realpath(join(root, entry.name))
    if (relative(root, path).startsWith(".." + sep) || !path.startsWith(root + sep)) throw new Error("License path escaped its package")
    const bytes = await readFile(path)
    if (bytes.length > 1024 * 1024 || bytes.includes(0)) throw new Error("Invalid dependency notice")
    notices.push(`===== ${label} / ${entry.name} =====\n${bytes.toString("utf8").trim()}\n`)
    names.push({ file: entry.name, sha256: digest(bytes) })
  }
  return names
}

const rust = []
const localGlib = join(projectRoot, "vendor", "glib")
const patch = JSON.parse(await readFile(join(localGlib, "RYO_PATCH_PROVENANCE.json"), "utf8"))
if (patch.modified_file !== "src/variant_iter.rs" || digest(await readFile(join(localGlib, patch.modified_file))) !== patch.patched_file_sha256) throw new Error("Reviewed GLib patch changed")
for (const item of cargo.packages.filter(item => item.source !== null || item.name === "glib").sort((a,b) => `${a.name}@${a.version}`.localeCompare(`${b.name}@${b.version}`))) {
  const patched = item.source === null && item.name === "glib"
  if (patched && (item.version !== patch.original_version || await realpath(dirname(item.manifest_path)) !== await realpath(localGlib))) throw new Error("Unexpected local GLib source")
  rust.push({ name: item.name, version: item.version, license: item.license ?? "NOASSERTION", source: patched ? "https://crates.io/crates/glib/0.18.5" : item.source, repository: item.repository,
    ...(patched ? { local_patch: { upstream_fix: patch.upstream_fix, upstream_fix_commit: patch.upstream_fix_commit, original_crates_io_sha256: patch.original_crates_io_sha256, modified_file: patch.modified_file, patched_file_sha256: patch.patched_file_sha256 } } : {}),
    notices: [...await licenseTexts(dirname(item.manifest_path), `Cargo ${item.name}@${item.version}`), ...await supplementaryNotices("cargo", item.name, item.version)] })
}
const frontend = []
for (const item of Object.values(npm).flat().filter(item => item.name !== "ryo-wallet-next").sort((a,b) => a.name.localeCompare(b.name))) {
  for (const directory of item.paths) {
    const manifest = JSON.parse(await readFile(join(directory, "package.json"), "utf8"))
    if (frontend.some(record => record.name === manifest.name && record.version === manifest.version)) continue
    frontend.push({ name: manifest.name, version: manifest.version, license: item.license ?? "NOASSERTION", notices: [...await licenseTexts(directory, `npm ${manifest.name}@${manifest.version}`), ...await supplementaryNotices("npm", manifest.name, manifest.version)] })
  }
}
if (usedSupplemental.size !== supplemental.records.length) throw new Error("Supplemental inventory contains a package outside the locked graph; review dependency changes")
const content = "Dependency notices from the reviewed Cargo and production npm lockfile graphs.\nIncludes dependencies for supported targets and Cargo build tools; this is not a claim that every listed component is statically linked.\nAvailable published notice text and reviewed publication-linked upstream supplements follow. Remaining packages without notice files are explicitly identified in dependency-inventory.json.\n\n" + notices.join("\n")
await writeFile(join(legalRoot, "DEPENDENCY_NOTICES.txt"), content)
const inventory = { schema_version: 1, scope: "All locked Cargo packages across targets and installed production npm dependencies. Ryo executable inventory and its upstream notices are separate; its internal static-library composition is not inferred.", lockfile_sha256: { "Cargo.lock": digest(await readFile(join(projectRoot,"Cargo.lock"))), "pnpm-lock.yaml": digest(await readFile(join(projectRoot,"pnpm-lock.yaml"))) }, notices_sha256: digest(Buffer.from(content)), cargo: rust, npm: frontend }
await writeFile(join(legalRoot, "dependency-inventory.json"), JSON.stringify(inventory, null, 2) + "\n")
process.stdout.write(JSON.stringify({ cargo_packages: rust.length, production_npm_packages: frontend.length, notice_files: notices.length, notice_bytes: Buffer.byteLength(content), packages_without_notice_files: [...rust,...frontend].filter(item => !item.notices.length).length }) + "\n")
