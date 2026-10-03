/** An advisory check; the backend's required password length is independent. */
export function isWeakPassword(password: string): boolean {
  return new TextEncoder().encode(password).length < 16 || /^(.{1,4})\1+$/.test(password) || /^(password|qwerty|123456|letmein)/i.test(password)
}
