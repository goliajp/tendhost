// Turns an entry into a sentence. This module's own reading of the events
// other modules publish: it looks at the topic and the few payload fields
// it needs, and falls back to the topic itself for anything it does not
// know, so an event from a module added later still shows up.

import type { Actor, Entry } from './api'

const str = (value: unknown): string | null => (typeof value === 'string' && value !== '' ? value : null)

const named = (payload: Entry['payload'], what: string): string => {
  const name = str(payload.name)
  return name ? `${what} ${name}` : what
}

const SENTENCES: Record<string, (payload: Entry['payload']) => string> = {
  'identity.owner_created': (p) => named(p, 'Created the owner account'),
  'identity.user_created': (p) => `${named(p, 'Added the user')}${str(p.role) ? ` as ${str(p.role)}` : ''}`,
  'identity.user_removed': (p) => named(p, 'Removed the user'),
  'identity.signed_in': () => 'Signed in',
  'identity.token_created': (p) => `${named(p, 'Made the API token')}${str(p.role) ? `, role ${str(p.role)}` : ''}`,
  'identity.token_revoked': (p) => named(p, 'Revoked the API token'),
  'host.enrollment_opened': () => 'Opened an enrollment for a new host',
  'host.enrolled': (p) =>
    `${named(p, 'Enrolled as')}${p.mode === 'manage' ? ', allowing commands' : p.mode === 'observe' ? ', reporting only' : ''}`,
  'host.updated': (p) => {
    const labels = Array.isArray(p.labels) ? p.labels.filter((l) => typeof l === 'string') : null
    const tail = labels === null ? '' : labels.length === 0 ? '; no labels' : `; labels ${labels.join(', ')}`
    return `${named(p, 'Set the host to')}${tail}`
  },
  'host.removed': (p) => named(p, 'Removed the host'),
}

export function describe(entry: Pick<Entry, 'topic' | 'payload'>): string {
  return SENTENCES[entry.topic]?.(entry.payload) ?? entry.topic
}

/** Who did it, as far as this module can tell: the audit log records ids. */
export function who(actor: Actor, selfId: string): string {
  switch (actor.kind) {
    case 'system':
      return 'The server'
    case 'host':
      return `Host ${actor.id}`
    case 'user':
      return actor.id === selfId ? 'You' : `User ${actor.id}`
  }
}

/** Whether `entries` (newest first) reach the beginning of the log. */
export function reachesStart(entries: readonly Pick<Entry, 'seq'>[], lastPageSize: number, page: number): boolean {
  const oldest = entries[entries.length - 1]
  return oldest === undefined || oldest.seq <= 1 || lastPageSize < page
}
