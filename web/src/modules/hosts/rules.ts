import type { Host, Mode } from './api'

export const MODE_NAME: Record<Mode, string> = {
  observe: 'Report only',
  manage: 'Report and run commands',
}

export const MODE_MEANING: Record<Mode, string> = {
  observe:
    'The machine sends its facts and measurements. The server cannot run anything on it, whatever happens to the server',
  manage:
    'The machine also runs the commands this server sends it. Anyone who gains admin access to the server can then run commands there',
}

/** Quotes a value for a posix shell only when it needs it. */
export function shellQuote(value: string): string {
  if (/^[A-Za-z0-9_\-.:/@=+]+$/.test(value)) return value
  return `'${value.replaceAll("'", `'\\''`)}'`
}

/** The command that joins a machine to the server at `origin`. */
export function enrollCommand(origin: string, token: string, mode: Mode): string {
  const manage = mode === 'manage' ? ' --manage' : ''
  return `tendhostd enroll --server ${shellQuote(origin)} --token ${shellQuote(token)}${manage}`
}

export const RUN_COMMAND = 'tendhostd run'

/** True when hosts would send their credential to `origin` unencrypted
 *  over a network: plain http to anything but this machine. */
export function sendsCredentialInClear(origin: string): boolean {
  const url = new URL(origin)
  return url.protocol === 'http:' && !['localhost', '127.0.0.1', '[::1]'].includes(url.hostname)
}

/** The hosts that were not there when the wizard started. */
export function joinedSince(before: readonly string[], now: readonly Host[]): Host[] {
  return now.filter((host) => !before.includes(host.id))
}

/** One label as typed, ready to add; null when it adds nothing. */
export function labelToAdd(typed: string, existing: readonly string[]): string | null {
  const label = typed.trim()
  return label === '' || existing.includes(label) ? null : label
}

export function byName(a: Host, b: Host): number {
  return a.name.localeCompare(b.name, undefined, { numeric: true })
}
