import { api } from '@/kit/api'

export type Kind = 'host_offline' | 'disk_usage' | 'agent_limit' | 'job_failed'

export interface Alert {
  id: string
  key: string
  kind: Kind
  host_id: string
  /** one sentence, e.g. `db-01 is offline` */
  summary: string
  detail: string
  opened_at: string
  resolved_at: string | null
}

/** Which conditions raise an alert. A threshold is a share from 0.01 to
 *  1; null turns the condition off. */
export interface Settings {
  host_offline: boolean
  disk_usage: number | null
  agent_limit: number | null
  job_failed: boolean
}

export const alertsApi = {
  open: () => api.get<Alert[]>('/alerts'),
  /** open and resolved, newest first, the last 200 */
  history: () => api.get<Alert[]>('/alerts?state=all'),
  settings: () => api.get<Settings>('/alerts/settings'),
  setSettings: (settings: Settings) => api.put<Settings>('/alerts/settings', settings),
}
