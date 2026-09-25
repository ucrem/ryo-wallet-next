import { describe, expect, it } from "vitest"
import { challengePositions, verifyBackupWords } from "./backupChallenge"

describe("recovery backup confirmation", () => {
  const phrase = "alpha bravo charlie delta echo foxtrot golf hotel india"

  it("requires words from the start, middle and end", () => {
    expect(challengePositions(phrase)).toEqual([0, 4, 8])
    expect(verifyBackupWords(phrase, { 0: "Alpha", 4: "echo", 8: "india" })).toBe(true)
    expect(verifyBackupWords(phrase, { 0: "alpha", 4: "echo", 8: "" })).toBe(false)
    expect(verifyBackupWords(phrase, { 0: "alpha", 4: "golf", 8: "india" })).toBe(false)
  })

  it("rejects a malformed short phrase", () => {
    expect(verifyBackupWords("one two", { 0: "one", 1: "two" })).toBe(false)
  })
})
