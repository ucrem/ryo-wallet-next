export function displayVersion(version: string): string {
  return `v${version}`
}

export function releaseStatus(version: string): string {
  if (/-alpha(?:\.|-|$)/i.test(version)) return "Alpha preview"
  if (/-beta(?:\.|-|$)/i.test(version)) return "Beta preview"
  if (/-rc(?:\.|-|$)/i.test(version)) return "Release candidate"
  return "Release"
}

export function releaseUrl(version: string): string | null {
  if (!/^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(version)) {
    return null
  }
  return `https://github.com/ucrem/ryo-wallet-next/releases/tag/v${version}`
}
