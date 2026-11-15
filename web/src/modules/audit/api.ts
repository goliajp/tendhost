import { api } from '@/kit/api'

export type Actor = { kind: 'user'; id: string } | { kind: 'host'; id: string } | { kind: 'system' }

export interface Entry {
  /** numbers the entries in the order they were recorded */
  seq: number
  id: string
  topic: string
  at: string
  actor: Actor
  payload: Record<string, unknown>
}

export const PAGE = 50

export const auditApi = {
  /** newest first; entries with `seq` below `before` */
  entries: (before?: number) =>
    api.get<Entry[]>(`/audit?limit=${PAGE}${before === undefined ? '' : `&before=${before}`}`),
}
