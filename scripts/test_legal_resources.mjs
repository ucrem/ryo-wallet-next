import assert from "node:assert/strict"
import { cp, mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { dirname, join } from "node:path"
import { test } from "node:test"
import { projectRoot, verifyLegalResources } from "./legal-resources.mjs"

async function fixture(t) {
  const root = await mkdtemp(join(tmpdir(), "ryo-notices-test-"))
  t.after(() => rm(root, { recursive: true, force: true }))
  for (const file of ["Cargo.lock", "pnpm-lock.yaml", "src-tauri/runtime-manifest.json", "vendor/glib/RYO_PATCH_PROVENANCE.json", "vendor/glib/src/variant_iter.rs"]) {
    await mkdir(dirname(join(root, file)), { recursive: true })
    await cp(join(projectRoot, file), join(root, file))
  }
  await cp(join(projectRoot, "src-tauri/resources/legal"), join(root, "src-tauri/resources/legal"), { recursive: true })
  return root
}

test("reviewed dependency and runtime notices pass packaging integrity checks", async () => {
  await verifyLegalResources()
})

test("a dependency change requires notice inventory regeneration", async (t) => {
  const root = await fixture(t)
  await writeFile(join(root, "Cargo.lock"), "changed dependency graph")
  await assert.rejects(verifyLegalResources(root), /inventory is stale/)
})

test("a runtime upgrade or damaged upstream notice blocks packaging", async (t) => {
  const root = await fixture(t)
  const path = join(root, "src-tauri/runtime-manifest.json")
  const original = await readFile(path)
  const runtime = JSON.parse(original)
  runtime.version = "unreviewed-upgrade"
  await writeFile(path, JSON.stringify(runtime))
  await assert.rejects(verifyLegalResources(root), /Runtime notices inventory is stale/)
  await writeFile(path, original)
  await writeFile(join(root, "src-tauri/resources/legal/RYO-LICENSE.txt"), "damaged notice")
  await assert.rejects(verifyLegalResources(root), /notice failed integrity/)
})

test("changed dependency notices and changed GLib patch are rejected", async (t) => {
  const root = await fixture(t)
  const path = join(root, "src-tauri/resources/legal/DEPENDENCY_NOTICES.txt")
  const original = await readFile(path)
  await writeFile(path, "damaged notices")
  await assert.rejects(verifyLegalResources(root), /Dependency notices failed integrity/)
  await writeFile(path, original)
  await writeFile(join(root, "vendor/glib/src/variant_iter.rs"), "unreviewed patch")
  await assert.rejects(verifyLegalResources(root), /GLib backport failed integrity/)
})
