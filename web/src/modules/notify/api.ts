import { api } from '@/kit/api'

export type Security = 'tls' | 'starttls' | 'none'

/** Where a channel sends, as it is created. Secrets go in and never come
 *  back out. */
export type Target =
  | { kind: 'webhook'; url: string }
  | { kind: 'ntfy'; url: string; token: string | null }
  | {
      kind: 'email'
      host: string
      port: number
      security: Security
      username: string | null
      password: string | null
      from: string
      to: string
    }

/** A target as the server shows it: whether a secret is set, not what it is. */
export type TargetView =
  | { kind: 'webhook'; url: string }
  | { kind: 'ntfy'; url: string; has_token: boolean }
  | {
      kind: 'email'
      host: string
      port: number
      security: Security
      username: string | null
      has_password: boolean
      from: string
      to: string
    }

export interface Channel {
  id: string
  name: string
  target: TargetView
  created_at: string
}

/** one attempt to send one message over one channel */
export interface Delivery {
  channel_id: string
  channel_name: string
  at: string
  title: string
  ok: boolean
  error: string | null
}

export const notifyApi = {
  channels: () => api.get<Channel[]>('/channels'),
  create: (name: string, target: Target) => api.post<Channel>('/channels', { name, target }),
  remove: (id: string) => api.delete(`/channels/${encodeURIComponent(id)}`),
  /** sends a test message now and says how it went */
  test: (id: string) => api.post<Delivery>(`/channels/${encodeURIComponent(id)}/test`),
  deliveries: () => api.get<Delivery[]>('/deliveries'),
}
