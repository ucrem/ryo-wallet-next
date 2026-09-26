import { createHash } from "node:crypto"
import { spawnSync } from "node:child_process"
import { createReadStream, createWriteStream } from "node:fs"
import { chmod, copyFile, lstat, mkdir, mkdtemp, readFile, rename, rm, writeFile } from "node:fs/promises"
import { dirname, join, resolve } from "node:path"
import { Readable } from "node:stream"
import { pipeline } from "node:stream/promises"
import { fileURLToPath } from "node:url"

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..")
const manifestPath = join(projectRoot, "src-tauri", "runtime-manifest.json")

export function hostTarget(platform = process.platform, arch = process.arch) {
  if (platform === "linux" && arch === "x64") return "x86_64-unknown-linux-gnu"
  if (platform === "darwin" && arch === "x64") return "x86_64-apple-darwin"
  if (platform === "win32" && arch === "x64") return "x86_64-pc-windows-msvc"
  return null
}

export async function sha256(path) {
  const hash = createHash("sha256")
  for await (const chunk of createReadStream(path)) hash.update(chunk)
  return hash.digest("hex")
}

async function verifiedFile(path, expected) {
  try {
    const metadata = await lstat(path)
    return metadata.isFile() && !metadata.isSymbolicLink() && await sha256(path) === expected
  } catch (error) {
    if (error?.code === "ENOENT") return false
    throw error
  }
}

async function stageDevelopmentSidecar(source, target, expected) {
  const directory = join(projectRoot, "src-tauri", "binaries")
  const destination = join(directory, `ryo-wallet-rpc-${target}${process.platform === "win32" ? ".exe" : ""}`)
  await mkdir(directory, { recursive: true, mode: 0o700 })
  if (await verifiedFile(destination, expected)) return
  await rm(destination, { force: true })
  await copyFile(source, destination)
  if (process.platform !== "win32") await chmod(destination, 0o755)
  if (!await verifiedFile(destination, expected)) throw new Error("Staged development sidecar failed verification")
}

function extractMember(archive, member) {
  const zip = archive.endsWith(".zip")
  const command = zip
    ? process.platform === "win32" ? join(process.env.ProgramFiles ?? "C:\\Program Files", "7-Zip", "7z.exe") : "7z"
    : "tar"
  const args = zip ? ["e", "-so", archive, member] : ["-xJOf", archive, member]
  const result = spawnSync(command, args, { maxBuffer: 128 * 1024 * 1024 })
  if (result.error || result.status !== 0 || !result.stdout?.length) {
    throw new Error("Could not extract the reviewed wallet RPC executable")
  }
  return result.stdout
}

export async function prepareWalletRuntime(mode) {
  if (mode !== "development" && mode !== "package") throw new Error("Invalid runtime preparation mode")
  const target = hostTarget()
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"))
  const reviewed = target && manifest.platforms[target]
  if (!reviewed) throw new Error(`No reviewed Ryo ${manifest.version} wallet RPC runtime for this platform`)

  const directory = join(projectRoot, "src-tauri", mode === "package" ? "binaries" : ".dev-runtime")
  const filename = mode === "package"
    ? `ryo-wallet-rpc-${target}${process.platform === "win32" ? ".exe" : ""}`
    : `ryo-wallet-rpc${process.platform === "win32" ? ".exe" : ""}`
  const destination = join(directory, filename)
  await mkdir(directory, { recursive: true, mode: 0o700 })
  // Development may reuse a verified copy. Package builds verify the archive
  // and extracted executable on every invocation.
  if (mode === "development" && await verifiedFile(destination, reviewed.binarySha256)) {
    await stageDevelopmentSidecar(destination, target, reviewed.binarySha256)
    return
  }

  const temporary = await mkdtemp(join(directory, "prepare-"))
  try {
    const archive = join(temporary, reviewed.archive)
    const url = `${manifest.releaseBase}/${reviewed.archive}`
    process.stdout.write(`Preparing verified Ryo ${manifest.version} wallet runtime for ${mode}…\n`)
    const response = await fetch(url)
    if (!response.ok || !response.body) throw new Error(`Ryo release download failed (HTTP ${response.status})`)
    await pipeline(Readable.fromWeb(response.body), createWriteStream(archive, { mode: 0o600 }))
    if (!await verifiedFile(archive, reviewed.archiveSha256)) {
      throw new Error("Ryo release archive SHA-256 did not match the reviewed manifest")
    }
    const extracted = join(temporary, filename)
    await writeFile(extracted, extractMember(archive, reviewed.member), { mode: 0o700 })
    if (!await verifiedFile(extracted, reviewed.binarySha256)) {
      throw new Error("Ryo wallet RPC SHA-256 did not match the reviewed manifest")
    }
    if (process.platform !== "win32") await chmod(extracted, 0o755)
    await rm(destination, { force: true })
    await rename(extracted, destination)
    if (mode === "development") await stageDevelopmentSidecar(destination, target, reviewed.binarySha256)
  } finally {
    await rm(temporary, { recursive: true, force: true })
  }
}
