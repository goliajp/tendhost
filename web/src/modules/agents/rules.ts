import type { AgentKind, AgentView, HostRef, UsageDay } from './api'

export const AGENT_NAME: Record<AgentKind, string> = {
  claude: 'Claude Code',
  codex: 'Codex',
  gemini: 'Gemini CLI',
  kimi: 'Kimi Code',
}

export const agentName = (agent: string): string => AGENT_NAME[agent as AgentKind] ?? agent

/** What tracking an agent will bring, said for exactly what it offers. */
export function offers(view: Pick<AgentView, 'supports_usage' | 'supports_limits'>): string {
  if (view.supports_usage && view.supports_limits) return 'Tracking brings its accounts, token usage per day and limit windows'
  if (view.supports_usage) {
    return 'Tracking brings its accounts and token usage per day. It keeps no limit readings where tendhostd can read them'
  }
  if (view.supports_limits) {
    return 'Tracking brings its accounts and limit windows. It keeps no usage where tendhostd can read it'
  }
  return 'Tracking brings its accounts only. It keeps neither usage nor limit readings where tendhostd can read them'
}

export interface HostGroup {
  host: HostRef
  agents: AgentView[]
}

/** Agents grouped under their host, hosts by name, agents by kind. A view
 *  whose host is unknown (removed a moment ago) is left out. */
export function byHost(views: readonly AgentView[], hosts: readonly HostRef[]): HostGroup[] {
  return hosts
    .map((host) => ({
      host,
      agents: views.filter((view) => view.host_id === host.id).sort((a, b) => a.agent.localeCompare(b.agent)),
    }))
    .filter((group) => group.agents.length > 0)
    .sort((a, b) => a.host.name.localeCompare(b.host.name, undefined, { numeric: true }))
}

/** The agents tracked on a host, with one added or removed. */
export function trackedAfter(views: readonly AgentView[], hostId: string, agent: AgentKind, track: boolean): AgentKind[] {
  const now = views.filter((view) => view.host_id === hostId && view.tracked).map((view) => view.agent)
  const next = track ? [...now, agent] : now.filter((kind) => kind !== agent)
  return [...new Set(next)].sort()
}

/** True once the daemon has answered for every agent in `wanted`: its
 *  detail (the accounts) is there. */
export function detailArrived(views: readonly AgentView[], hostId: string, wanted: readonly AgentKind[]): boolean {
  return wanted.every((agent) =>
    views.some((view) => view.host_id === hostId && view.agent === agent && view.tracked && view.accounts.length > 0),
  )
}

/** `950`, `1.2k`, `3.4M`, `5.6B` */
export function compact(n: number): string {
  if (n < 1000) return String(n)
  for (const [size, unit] of [
    [1e9, 'B'],
    [1e6, 'M'],
    [1e3, 'k'],
  ] as const) {
    if (n >= size) return `${(n / size).toFixed(n / size >= 100 ? 0 : 1)}${unit}`
  }
  return String(n)
}

export interface Totals {
  input: number
  output: number
  cacheRead: number
  cacheWrite: number
}

export function totals(days: readonly UsageDay[]): Totals {
  return days.reduce(
    (sum, day) => ({
      input: sum.input + day.input_tokens,
      output: sum.output + day.output_tokens,
      cacheRead: sum.cacheRead + day.cache_read_tokens,
      cacheWrite: sum.cacheWrite + day.cache_write_tokens,
    }),
    { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
  )
}

/** what one bar segment stands for: tokens sent and received, without
 *  cache traffic, which would dwarf them */
export const worked = (day: UsageDay): number => day.input_tokens + day.output_tokens

export const OTHER = 'other models'

/** The models to draw as their own series: the `max - 1` that worked
 *  most, then everything else as one. Order is by tokens, largest first. */
export function seriesModels(days: readonly UsageDay[], max: number): string[] {
  const sums = new Map<string, number>()
  for (const day of days) sums.set(day.model, (sums.get(day.model) ?? 0) + worked(day))
  const ranked = [...sums.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])).map(([model]) => model)
  return ranked.length <= max ? ranked : [...ranked.slice(0, max - 1), OTHER]
}

export interface DayStack {
  day: string
  /** tokens per series, in the order of `models` */
  parts: number[]
  total: number
}

/** `YYYY-MM-DD` for each of the last `count` days ending at `today` (UTC,
 *  as the server keys usage). */
export function lastDays(today: Date, count: number): string[] {
  return Array.from({ length: count }, (_, i) => {
    const day = new Date(Date.UTC(today.getUTCFullYear(), today.getUTCMonth(), today.getUTCDate() - (count - 1 - i)))
    return day.toISOString().slice(0, 10)
  })
}

/** One stack per day of the span; a day without records is a day without
 *  sessions, so its stack is zero. */
export function stacks(days: readonly UsageDay[], span: readonly string[], models: readonly string[]): DayStack[] {
  return span.map((day) => {
    const parts = models.map(() => 0)
    for (const record of days) {
      if (record.day !== day) continue
      const index = models.includes(record.model) ? models.indexOf(record.model) : models.indexOf(OTHER)
      if (index >= 0) parts[index]! += worked(record)
    }
    return { day, parts, total: parts.reduce((a, b) => a + b, 0) }
  })
}
