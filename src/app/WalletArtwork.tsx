import QRCode from "qrcode"
import type { Artwork } from "@/api/operations"

export function qrArtwork(value: string): Artwork {
  const matrix = QRCode.create(value, { errorCorrectionLevel: "M" }).modules
  const cells: [number, number][] = []
  for (let y = 0; y < matrix.size; y++) for (let x = 0; x < matrix.size; x++) {
    if (matrix.get(y, x)) cells.push([x + 4, y + 4])
  }
  return { size: matrix.size + 8, cells, color: "#000000" }
}
export function identiconArtwork(address: string): Artwork {
  let seed = 2166136261
  for (const char of address) seed = Math.imul(seed ^ char.charCodeAt(0), 16777619) >>> 0
  const color = `#${((seed & 0x7f7f7f) | 0x404040).toString(16).padStart(6, "0")}`
  const cells: [number, number][] = []
  for (let y = 0; y < 5; y++) for (let x = 0; x < 3; x++) {
    seed ^= seed << 13; seed ^= seed >>> 17; seed ^= seed << 5
    if (seed & 1) { cells.push([x, y]); if (x < 2) cells.push([4 - x, y]) }
  }
  return { size: 5, cells, color }
}
export function WalletArtwork({ artwork, label, className = "size-48" }: { artwork: Artwork; label: string; className?: string }) {
  return <svg role="img" aria-label={label} viewBox={`0 0 ${artwork.size} ${artwork.size}`} className={className} shapeRendering="crispEdges">
    <rect width="100%" height="100%" fill="white" />
    <g fill={artwork.color}>{artwork.cells.map(([x, y]) => <rect key={`${x}-${y}`} x={x} y={y} width="1" height="1" />)}</g>
  </svg>
}
