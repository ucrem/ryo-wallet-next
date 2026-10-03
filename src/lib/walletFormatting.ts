export function formatRyo(atomic: string): string {
  const value = BigInt(atomic)
  const whole = value / 1_000_000_000n
  const fraction = (value % 1_000_000_000n).toString().padStart(9, "0").replace(/0+$/, "")
  return fraction ? `${whole}.${fraction}` : whole.toString()
}
export function exactAmount(value: string): boolean {
  if (!/^\d+(\.\d{1,9})?$/.test(value)) return false
  const [whole, fraction = ""] = value.split(".")
  const atomic = BigInt(whole) * 1_000_000_000n + BigInt(fraction.padEnd(9, "0"))
  return atomic > 0n && atomic <= 18_446_744_073_709_551_615n
}
export function transactionDate(timestamp: string): string {
  const value = Number(timestamp)
  if (!Number.isSafeInteger(value) || value <= 0) return "Pending"
  return new Date(value * 1000).toLocaleString()
}
