// Probe only a fresh testnet node. No wallet, credentials or mainnet paths.
import { spawn } from "node:child_process"
import console from "node:console"
import { createHash } from "node:crypto"
import { lookup } from "node:dns/promises"
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises"
import { createConnection, createServer } from "node:net"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { setTimeout as delay } from "node:timers/promises"

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..")
const observationSeconds = Number(process.argv[2] ?? 60)
if (!Number.isInteger(observationSeconds) || observationSeconds < 1 || observationSeconds > 300) {
  throw new Error("Usage: node scripts/probe-testnet.mjs [observation seconds: 1..300]")
}
const target = process.platform === "win32" ? "x86_64-pc-windows-msvc"
  : process.platform === "darwin" ? "x86_64-apple-darwin" : "x86_64-unknown-linux-gnu"
if (process.arch !== "x64") throw new Error("No reviewed native runtime for this architecture")
const manifest = JSON.parse(await readFile(join(root, "src-tauri/runtime-manifest.json"), "utf8"))
const binary = resolve(process.env.RYO_TEST_DAEMON ?? join(root, "src-tauri/.dev-runtime", process.platform === "win32" ? "ryod.exe" : "ryod"))
const digest = createHash("sha256").update(await readFile(binary)).digest("hex")
if (digest !== manifest.platforms[target].daemonSha256) throw new Error("Daemon digest differs from reviewed manifest")
const reportRoot = join(root, "target/testnet-validation")
await mkdir(reportRoot, { recursive: true })
const directory = await mkdtemp(join(reportRoot, "probe-"))
const config = join(directory, "empty.conf")
await writeFile(config, "", { flag: "wx", mode: 0o600 })
const listeners = await Promise.all(Array.from({ length: 3 }, () => new Promise((ok, fail) => {
  const server = createServer()
  server.once("error", fail)
  server.listen(0, "127.0.0.1", () => ok(server))
})))
const [rpcPort, p2pPort, zmqPort] = listeners.map(server => server.address().port)
await Promise.all(listeners.map(server => new Promise(ok => server.close(ok))))
const sleep = ms => delay(ms, undefined, { ref: false })
const seeds = ["185.134.22.134", "81.19.208.43", "149.56.44.109"]
const atomPeer = "45.77.68.151"
const reachability = await Promise.all([...seeds, atomPeer].map(host => new Promise(ok => {
  const socket = createConnection({ host, port: 13310 })
  const finish = reachable => { socket.destroy(); ok({ host, port: 13310, tcpReachable: reachable }) }
  socket.setTimeout(3000, () => finish(false))
  socket.once("connect", () => finish(true))
  socket.once("error", () => finish(false))
})))
const explorerDns = await lookup("tnexp.ryo-currency.com").then(() => "resolved", () => "unavailable")
const child = spawn(binary, ["--testnet", "--config-file", config, "--data-dir", directory,
  "--add-peer", `${atomPeer}:13310`,
  "--rpc-bind-ip", "127.0.0.1", "--rpc-bind-port", String(rpcPort), "--restricted-rpc",
  "--p2p-bind-ip", "127.0.0.1", "--p2p-bind-port", String(p2pPort), "--hide-my-port", "--no-igd",
  "--zmq-rpc-bind-ip", "127.0.0.1", "--zmq-rpc-bind-port", String(zmqPort),
  "--check-updates", "disabled", "--log-file", join(directory, "daemon.log"), "--log-level", "0"],
{ cwd: directory, windowsHide: true, stdio: ["pipe", "ignore", "ignore"] })
const closed = new Promise(ok => child.once("close", code => ok(code)))
let startupError
child.once("error", error => { startupError = error })
async function info() {
  const response = await fetch(`http://127.0.0.1:${rpcPort}/json_rpc`, {
    method: "POST", headers: { "content-type": "application/json" }, redirect: "error",
    body: JSON.stringify({ jsonrpc: "2.0", id: "probe", method: "get_info", params: {} }),
    signal: globalThis.AbortSignal.timeout(3000),
  })
  const body = await response.json()
  if (!response.ok || body.error || !body.result?.testnet || body.result.mainnet || body.result.stagenet) {
    throw new Error("Daemon did not report the expected testnet")
  }
  const r = body.result
  return { height: r.height, targetHeight: r.target_height, difficulty: r.difficulty,
    ready: r.is_ready, outgoing: r.outgoing_connections_count, incoming: r.incoming_connections_count,
    topHash: r.top_block_hash }
}
const samples = []
const report = { observedAt: new Date().toISOString(), runtime: manifest.version, daemonSha256: digest,
  upstreamSeedSource: "166cf188bf351e3eecff904450eda2785f9d0357/src/p2p/net_node.inl",
  atomPeerSource: "6c8d0aa68245271fe0e781084b38583abf758869/src-electron/main-process/modules/daemon.js",
  observationSeconds, seeds: reachability, explorerDns, samples, publicSyncObserved: false }
try {
  const until = Date.now() + 30000
  while (true) {
    if (startupError) throw startupError
    if (child.exitCode !== null) throw new Error("Testnet daemon exited during startup")
    try { samples.push(await info()); break } catch {
      if (Date.now() >= until) throw new Error("Testnet RPC startup timed out")
      await sleep(250)
    }
  }
  const deadline = Date.now() + observationSeconds * 1000
  while (Date.now() < deadline) {
    await sleep(Math.min(5000, deadline - Date.now()))
    samples.push(await info())
  }
  report.publicSyncObserved = samples.some(sample => sample.height > 1 && sample.outgoing > 0)
} catch (error) {
  report.error = error.message
  process.exitCode = 1
} finally {
  if (child.exitCode === null) {
    child.stdin.on("error", () => {})
    child.stdin.end("exit\n")
    const exited = await Promise.race([closed.then(() => true), sleep(15000).then(() => false)])
    if (!exited) child.kill()
  }
  report.exitCode = await closed
  const reportPath = join(directory, "report.json")
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`)
  console.log(JSON.stringify({ reportPath, ...report }, null, 2))
}
