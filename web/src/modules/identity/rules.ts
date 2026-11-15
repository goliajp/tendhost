import type { Role } from '@/kit/session'

export const MIN_PASSWORD_CHARS = 10

/** the server counts characters, not bytes or utf-16 units */
export function passwordProblem(password: string, confirm: string): { password: string | null; confirm: string | null } {
  const tooShort = [...password].length < MIN_PASSWORD_CHARS
  return {
    password: tooShort ? `Use at least ${MIN_PASSWORD_CHARS} characters` : null,
    confirm: !tooShort && password !== confirm ? 'The two passwords differ' : null,
  }
}

/** the roles someone with `role` may hand out: none above their own */
export function grantable(role: Role): Role[] {
  const all: Role[] = ['viewer', 'admin', 'owner']
  return all.slice(0, all.indexOf(role) + 1)
}

export const ROLE_MEANING: Record<Role, string> = {
  viewer: 'Sees hosts and their measurements',
  admin: 'Also adds and changes hosts, users and tokens, and reads the audit log',
  owner: 'Also creates and removes other owners',
}
