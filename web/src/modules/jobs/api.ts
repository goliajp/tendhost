import { api } from '@/kit/api'

export type Schedule = { kind: 'manual' } | { kind: 'once'; at: string } | { kind: 'cron'; expr: string }

export type Actor = { kind: 'user'; id: string } | { kind: 'host'; id: string } | { kind: 'system' }

export interface Job {
  id: string
  name: string
  host_id: string
  command: string
  timeout_secs: number
  schedule: Schedule
  enabled: boolean
  created_at: string
  /** absent for a manual job, a disabled one, and a one-time job that ran */
  next_run_at: string | null
  /** how the newest finished run ended; null before any */
  last_run: { state: RunState; finished_at: string } | null
}

export interface NewJob {
  name: string
  host_id: string
  command: string
  timeout_secs: number
  schedule: Schedule
}

export type RunState = 'pending' | 'running' | 'succeeded' | 'failed' | 'timed_out' | 'missed' | 'lost'

export interface Run {
  id: string
  job_id: string
  host_id: string
  state: RunState
  due_at: string
  requested_by: Actor
  started_at: string | null
  finished_at: string | null
  exit_code: number | null
  /** the end of what the command printed */
  output: string
}

/** What this module needs to know of a host: its name, and whether it
 *  lets the server run commands. */
export interface HostRef {
  id: string
  name: string
  mode: 'observe' | 'manage'
}

const at = (id: string) => `/jobs/${encodeURIComponent(id)}`

export const jobsApi = {
  list: (hostId?: string) => api.get<Job[]>(hostId ? `/jobs?host=${encodeURIComponent(hostId)}` : '/jobs'),
  one: (id: string) => api.get<Job>(at(id)),
  create: (job: NewJob) => api.post<Job>('/jobs', job),
  remove: (id: string) => api.delete(at(id)),
  enable: (id: string) => api.post<Job>(`${at(id)}/enable`),
  disable: (id: string) => api.post<Job>(`${at(id)}/disable`),
  runs: (id: string) => api.get<Run[]>(`${at(id)}/runs`),
  runNow: (id: string) => api.post<Run>(`${at(id)}/runs`),
  hosts: () => api.get<HostRef[]>('/hosts'),
}
