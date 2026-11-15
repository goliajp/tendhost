import { api } from '@/kit/api'

export type Mode = 'observe' | 'manage'

export interface Facts {
  hostname: string
  os: string
  os_version: string
  arch: string
  cpu_count: number
  memory_bytes: number
  daemon_version: string
}

export interface Host {
  id: string
  name: string
  mode: Mode
  labels: string[]
  enrolled_at: string
  /** the enrollment that admitted it */
  enrollment_id: string
  facts: Facts
  facts_at: string
}

/** an enrollment at the one moment its token is known */
export interface NewEnrollment {
  id: string
  token: string
  expires_at: string
}

export const hostsApi = {
  list: () => api.get<Host[]>('/hosts'),
  one: (id: string) => api.get<Host>(`/hosts/${encodeURIComponent(id)}`),
  update: (id: string, changes: { name?: string; labels?: string[] }) =>
    api.patch<Host>(`/hosts/${encodeURIComponent(id)}`, changes),
  remove: (id: string) => api.delete(`/hosts/${encodeURIComponent(id)}`),
  openEnrollment: () => api.post<NewEnrollment>('/enrollments'),
}
