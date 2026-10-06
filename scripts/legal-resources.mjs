import { createHash } from "node:crypto"
import { readFile } from "node:fs/promises"
import { fileURLToPath } from "node:url"
import { dirname, join, resolve } from "node:path"

export const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..")
export const legalRoot = join(projectRoot, "src-tauri", "resources", "legal")
export const digest = (bytes) => createHash("sha256").update(bytes).digest("hex")

export async function verifyLegalResources(root = projectRoot) {
  const legal = join(root, "src-tauri", "resources", "legal")
  const runtime = JSON.parse(await readFile(join(root, "src-tauri", "runtime-manifest.json"), "utf8"))
  const inventory = JSON.parse(await readFile(join(legal, "runtime-inventory.json"), "utf8"))
  if (inventory.version !== runtime.version || inventory.release_base !== runtime.releaseBase || JSON.stringify(inventory.platforms) !== JSON.stringify(runtime.platforms)) {
    throw new Error("Runtime notices inventory is stale; review the new runtime before packaging")
  }
  if (inventory.license_files.map(item => item.file).sort().join(",") !== "RYO-LICENSE.txt,RYO-ORIGINAL-LICENSE.txt") {
    throw new Error("Both upstream Ryo notice files are required")
  }
  for (const notice of inventory.license_files) {
    if (!/^RYO-[A-Z-]+\.txt$/.test(notice.file) || digest(await readFile(join(legal, notice.file))) !== notice.sha256) {
      throw new Error("Pinned runtime notice failed integrity verification")
    }
  }
  const dependencies = JSON.parse(await readFile(join(legal, "dependency-inventory.json"), "utf8"))
  const supplemental = JSON.parse(await readFile(join(legal, "supplemental-inventory.json"), "utf8"))
  for (const record of supplemental.records) {
    const dependency = dependencies[record.ecosystem]?.find(item => item.name === record.name && item.version === record.version)
    if (!dependency || !/^[a-f0-9]{40}$/.test(record.source_commit)) throw new Error("Supplemental notice package/provenance does not match the locked inventory")
    for (const notice of record.notices) {
      if (!/^[a-f0-9]{64}\.txt$/.test(notice.file) ||
          digest(await readFile(join(legal, "supplemental", notice.file))) !== notice.sha256 ||
          !dependency.notices.some(item => item.file === `supplemental/${notice.file}` && item.sha256 === notice.sha256 && item.source === notice.source)) {
        throw new Error("Supplemental notice failed integrity verification")
      }
    }
  }
  const sourceMaterials = JSON.parse(await readFile(join(legal, "ryo-source-materials.json"), "utf8"))
  if (sourceMaterials.source_commit !== inventory.source_commit) throw new Error("Ryo source notice inventory is stale")
  for (const notice of sourceMaterials.notices) {
    if (!/^[a-f0-9]{64}\.txt$/.test(notice.file) || digest(await readFile(join(legal, "ryo-source-notices", notice.file))) !== notice.sha256) {
      throw new Error("Ryo source notice failed integrity verification")
    }
  }
  if (Object.keys(dependencies.lockfile_sha256).sort().join(",") !== "Cargo.lock,pnpm-lock.yaml") {
    throw new Error("Dependency notices must cover both lockfiles")
  }
  for (const [file, expected] of Object.entries(dependencies.lockfile_sha256)) {
    if (!["Cargo.lock", "pnpm-lock.yaml"].includes(file) || digest(await readFile(join(root, file))) !== expected) {
      throw new Error("Dependency notices inventory is stale; run node scripts/generate-legal-inventory.mjs after reviewing dependency changes")
    }
  }
  if (digest(await readFile(join(legal, "DEPENDENCY_NOTICES.txt"))) !== dependencies.notices_sha256) {
    throw new Error("Dependency notices failed integrity verification")
  }
  const patch = JSON.parse(await readFile(join(root, "vendor", "glib", "RYO_PATCH_PROVENANCE.json"), "utf8"))
  const glib = dependencies.cargo.find(item => item.name === "glib" && item.version === patch.original_version)
  if (patch.modified_file !== "src/variant_iter.rs" || !glib?.local_patch ||
      JSON.stringify(glib.local_patch) !== JSON.stringify({ upstream_fix: patch.upstream_fix, upstream_fix_commit: patch.upstream_fix_commit, original_crates_io_sha256: patch.original_crates_io_sha256, modified_file: patch.modified_file, patched_file_sha256: patch.patched_file_sha256 }) ||
      digest(await readFile(join(root, "vendor", "glib", patch.modified_file))) !== patch.patched_file_sha256) {
    throw new Error("Reviewed GLib backport failed integrity verification")
  }
}
