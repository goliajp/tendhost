import { api } from '@/kit/api'

export interface Presence {
  host_id: string
  last_seen: string
  online: boolean
}

export interface Disk {
  mount: string
  total_bytes: number
  used_bytes: number
}

export interface Sample {
  at: string
  load1: number
  memory_used_bytes: number
  uptime_secs: number
  disks: Disk[]
}

export const telemetryApi = {
  presence: () => api.get<Presence[]>('/presence'),
  samples: (hostId: string, since: Date) =>
    api.get<Sample[]>(`/hosts/${encodeURIComponent(hostId)}/samples?since=${encodeURIComponent(since.toISOString())}`),
}
