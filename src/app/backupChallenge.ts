export function challengePositions(phrase: string): number[] {
  const words = phrase.trim().split(/\s+/)
  if (words.length < 3) return []
  return [...new Set([0, Math.floor(words.length / 2), words.length - 1])]
}

export function verifyBackupWords(phrase: string, answers: Record<number, string>): boolean {
  const words = phrase.trim().split(/\s+/)
  const positions = challengePositions(phrase)
  return positions.length === 3 && positions.every((index) =>
    (answers[index] ?? "").trim().toLocaleLowerCase("en") === words[index].toLocaleLowerCase("en"))
}
