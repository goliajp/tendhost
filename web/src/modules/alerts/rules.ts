import type { Kind, Settings } from './api'

export const KIND_NAME: Record<Kind, string> = {
  host_offline: 'Host offline',
  disk_usage: 'Disk filling up',
  agent_limit: 'Agent limit nearly used',
  job_failed: 'Job failed',
}

/** The settings as the form holds them: thresholds in percent, as typed. */
export interface Form {
  hostOffline: boolean
  jobFailed: boolean
  diskOn: boolean
  diskPercent: string
  agentOn: boolean
  agentPercent: string
}

const shown = (ratio: number | null, fallback: number) => String(Math.round((ratio ?? fallback) * 100))

export function toForm(settings: Settings): Form {
  return {
    hostOffline: settings.host_offline,
    jobFailed: settings.job_failed,
    diskOn: settings.disk_usage !== null,
    diskPercent: shown(settings.disk_usage, 0.9),
    agentOn: settings.agent_limit !== null,
    agentPercent: shown(settings.agent_limit, 0.9),
  }
}

/** A typed percentage as a share; null when it is not a whole number
 *  from 1 to 100. */
export function ratioOf(percent: string): number | null {
  if (!/^\d{1,3}$/.test(percent.trim())) return null
  const n = Number(percent.trim())
  return n >= 1 && n <= 100 ? n / 100 : null
}

export type Parsed = { ok: true; settings: Settings } | { ok: false; field: 'disk_usage' | 'agent_limit' }

/** The form as settings, or the first threshold that is not a percentage. */
export function fromForm(form: Form): Parsed {
  const disk = form.diskOn ? ratioOf(form.diskPercent) : null
  if (form.diskOn && disk === null) return { ok: false, field: 'disk_usage' }
  const agent = form.agentOn ? ratioOf(form.agentPercent) : null
  if (form.agentOn && agent === null) return { ok: false, field: 'agent_limit' }
  return {
    ok: true,
    settings: { host_offline: form.hostOffline, disk_usage: disk, agent_limit: agent, job_failed: form.jobFailed },
  }
}
