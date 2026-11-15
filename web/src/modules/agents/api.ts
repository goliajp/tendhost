import { api } from '@/kit/api'

export type AgentKind = 'claude' | 'codex' | 'gemini' | 'kimi'

export interface Account {
  dir: string
  label: string
  plan: string | null
}

export interface LimitWindow {
  name: string
  /** 0 to 1 */
  used_ratio: number
  resets_at: string | null
}

export interface LimitReading {
  observed_at: string
  plan: string | null
  windows: LimitWindow[]
}

/** One agent on one host. */
export interface AgentView {
  host_id: string
  agent: AgentKind
  account_count: number
  supports_usage: boolean
  supports_limits: boolean
  seen_at: string
  tracked: boolean
  /** empty until tracked and reported */
  accounts: Account[]
  /** absent when not tracked, when the agent does not keep its limits
   *  where the daemon can read them, or before the first reading */
  limits: LimitReading | null
}

export interface UsageDay {
  /** `YYYY-MM-DD` */
  day: string
  model: string
  input_tokens: number
  output_tokens: number
  cache_read_tokens: number
  cache_write_tokens: number
}

/** What this module needs to know of a host. */
export interface HostRef {
  id: string
  name: string
}

export const agentsApi = {
  overview: () => api.get<AgentView[]>('/agents'),
  /** replaces the set of agents tracked on the host */
  setTracked: (hostId: string, tracked: AgentKind[]) =>
    api.put<{ tracked: AgentKind[] }>(`/hosts/${encodeURIComponent(hostId)}/agents`, { tracked }),
  usage: (hostId: string, agent: AgentKind, days: number) =>
    api.get<UsageDay[]>(`/hosts/${encodeURIComponent(hostId)}/agents/${agent}/usage?days=${days}`),
  hosts: () => api.get<HostRef[]>('/hosts'),
}
