import jsQR from "jsqr"
import { describe, expect, it } from "vitest"
import { qrArtwork } from "./WalletArtwork"

describe("receive QR", () => {
  it("decodes the exact request from the exported matrix with its quiet zone", () => {
    const request = "ryo:RYoL-public-address?tx_amount=9007199.254740993&tx_description=Invoice%20test"
    const artwork = qrArtwork(request)
    const scale = 6
    const width = artwork.size * scale
    const pixels = new Uint8ClampedArray(width * width * 4).fill(255)
    for (const [x, y] of artwork.cells) for (let dy = 0; dy < scale; dy++) for (let dx = 0; dx < scale; dx++) {
      const offset = ((y * scale + dy) * width + x * scale + dx) * 4
      pixels[offset] = pixels[offset + 1] = pixels[offset + 2] = 0
    }
    expect(jsQR(pixels, width, width)?.data).toBe(request)
  })
})
