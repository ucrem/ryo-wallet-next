import { describe, expect, it } from "vitest"
import { isWeakPassword } from "./passwordStrength"
describe("advisory password strength", () => {
  it("warns about short, repeated and common passwords without storing their values", () => {
    for (const password of ["abc123abc123", "xxxxxxxxxxxxxxxxxxxx", "password-very-long"]) expect(isWeakPassword(password)).toBe(true)
    expect(isWeakPassword("A distinct long test phrase 42")).toBe(false)
  })
})
