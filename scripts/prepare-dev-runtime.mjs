import { createHash } from "node:crypto"
import { createReadStream } from "node:fs"
import { chmod, mkdir, mkdtemp, rename, rm } from "node:fs/promises"
import { Readable } from "node:stream"
import { pipeline } from "node:stream/promises"
import { spawnSync } from "node:child_process"
import { fileURLToPath } from "node:url"
import { dirname, join, resolve } from "node:path"
import { createWriteStream } from "node:fs"

// The SHA-256 values identify the signed upstream 0.6.1.0 Linux release
// previously reviewed by this project. This file is only used in Tauri dev.
const archiveName = "ryo-linux-x64-0.6.1.0.tar.xz"
const archiveSha256 = "2b4de6e605735b2c2d7c851685bf858e6d0ba4132861147d1dfb3632e36b8b44"
const binarySha256 = "5ef7395ce822a02905e68a63abf6f3a9d75654c8a9773c23777902b5522169e3"
const releaseUrl = `https://github.com/ryo-currency/ryo-currency/releases/download/0.6.1.0/${archiveName}`
const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..")
const runtimeDir = join(projectRoot, "src-tauri", ".dev-runtime")
const binaryPath = join(runtimeDir, "ryo-wallet-rpc")

async function digest(path) {
  const hash = createHash("sha256")
  for await (const chunk of createReadStream(path)) hash.update(chunk)
  return hash.digest("hex")
}

async function matches(path, expected) {
  try {
    return (await digest(path)) === expected
  } catch (error) {
    if (error?.code === "ENOENT") return false
    throw error
  }
}

async function prepare() {
  if (process.platform !== "linux" || process.arch !== "x64") return
  await mkdir(runtimeDir, { recursive: true, mode: 0o700 })
  if (await matches(binaryPath, binarySha256)) return

  const temporary = await mkdtemp(join(runtimeDir, "prepare-"))
  try {
    const archivePath = join(temporary, archiveName)
    process.stdout.write("Preparing verified Ryo wallet runtime for local development…\n")
    const response = await fetch(releaseUrl)
    if (!response.ok || !response.body) {
      throw new Error(`Ryo release download failed (HTTP ${response.status})`)
    }
    await pipeline(Readable.fromWeb(response.body), createWriteStream(archivePath, { mode: 0o600 }))
    if (!(await matches(archivePath, archiveSha256))) {
      throw new Error("Ryo release archive SHA-256 did not match the reviewed release")
    }
    const extracted = join(temporary, "ryo-linux-x64-0.6.1.0", "ryo-wallet-rpc")
    const result = spawnSync("tar", ["-xJf", archivePath, "-C", temporary,
      "ryo-linux-x64-0.6.1.0/ryo-wallet-rpc"], { stdio: "pipe" })
    if (result.error || result.status !== 0) {
      throw new Error("Could not extract the reviewed Ryo wallet runtime")
    }
    if (!(await matches(extracted, binarySha256))) {
      throw new Error("Ryo wallet runtime SHA-256 did not match the reviewed binary")
    }
    await chmod(extracted, 0o700)
    await rename(extracted, binaryPath)
  } finally {
    await rm(temporary, { recursive: true, force: true })
  }
}

try {
  await prepare()
} catch (error) {
  process.stderr.write(`Local wallet runtime unavailable: ${error.message}\n`)
  process.exitCode = 1
}
