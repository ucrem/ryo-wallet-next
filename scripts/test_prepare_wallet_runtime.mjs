import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import { test } from "node:test"
import { URL } from "node:url"
import { hostTarget } from "./prepare-wallet-runtime.mjs"

const manifest = JSON.parse(await readFile(new URL("../src-tauri/runtime-manifest.json", import.meta.url), "utf8"))

test("only reviewed host targets can prepare a wallet runtime", () => {
  assert.equal(hostTarget("linux", "x64"), "x86_64-unknown-linux-gnu")
  assert.equal(hostTarget("darwin", "x64"), "x86_64-apple-darwin")
  assert.equal(hostTarget("win32", "x64"), "x86_64-pc-windows-msvc")
  assert.equal(hostTarget("darwin", "arm64"), null)
  assert.deepEqual(Object.keys(manifest.platforms).sort(), [
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
  ])
})

test("reviewed archive and executable digests are complete", () => {
  for (const runtime of Object.values(manifest.platforms)) {
    for (const key of ["archiveSha256", "binarySha256"]) {
      assert.match(runtime[key], /^[0-9a-f]{64}$/)
    }
    assert.ok(runtime.archive)
    assert.ok(runtime.member)
  }
})
