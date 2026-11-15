import { createContext, useContext, useMemo, type ReactNode } from 'react'
import { ago, parseTime } from '@/kit/format'
import { useNow, useResource } from '@/kit/use-resource'
import { telemetryApi, type Presence } from './api'

const REFRESH_MS = 10_000

interface PresenceState {
  /** undefined until the first answer */
  byHost: Map<string, Presence> | undefined
  failed: boolean
}

const PresenceContext = createContext<PresenceState>({ byHost: undefined, failed: false })

/** One poll of presence for the whole app, however many rows show it. */
export function PresenceProvider({ children }: { children: ReactNode }) {
  const presence = useResource('presence', telemetryApi.presence, REFRESH_MS)
  const value = useMemo<PresenceState>(
    () => ({
      byHost: presence.data && new Map(presence.data.map((p) => [p.host_id, p])),
      failed: presence.error !== undefined,
    }),
    [presence.data, presence.error],
  )
  return <PresenceContext.Provider value={value}>{children}</PresenceContext.Provider>
}

export type Standing = 'online' | 'offline' | 'never' | 'unknown'

/** What is known of one host: `never` when the server has not heard from
 *  it, `unknown` while presence has not loaded. */
export function standingOf(byHost: Map<string, Presence> | undefined, hostId: string): Standing {
  if (byHost === undefined) return 'unknown'
  const presence = byHost.get(hostId)
  if (!presence) return 'never'
  return presence.online ? 'online' : 'offline'
}

/** Fills `host.state`: whether the host answers, and when it last did. */
export function HostState({ hostId }: { hostId: string }) {
  const { byHost } = useContext(PresenceContext)
  const now = useNow()
  const standing = standingOf(byHost, hostId)
  const heard = ago(parseTime(byHost?.get(hostId)?.last_seen), now)

  const mark =
    standing === 'online'
      ? 'bg-live border-live'
      : standing === 'offline'
        ? 'bg-transparent border-down'
        : 'bg-transparent border-rule'
  const words =
    standing === 'online'
      ? `Online, heard ${heard}`
      : standing === 'offline'
        ? `Offline, last heard ${heard}`
        : standing === 'never'
          ? 'Never heard from'
          : '—'

  return (
    <span className="inline-flex items-baseline gap-2">
      <span aria-hidden className={`inline-block h-2.5 w-2.5 translate-y-[1px] rounded-[2px] border-2 ${mark}`} />
      <span className={`tabular ${standing === 'offline' ? 'text-down' : standing === 'online' ? '' : 'text-muted'}`}>
        {words}
      </span>
    </span>
  )
}

/** Fills `hosts.summary`: how many of the listed hosts answer. */
export function HostsSummary({ hostIds }: { hostIds: string[] }) {
  const { byHost, failed } = useContext(PresenceContext)
  if (byHost === undefined) return failed ? <>Whether they are online could not be loaded</> : null
  const online = hostIds.filter((id) => standingOf(byHost, id) === 'online').length
  return <>{summary(online, hostIds.length)}</>
}

export function summary(online: number, total: number): string {
  if (total === 0) return ''
  if (online === total) return total === 1 ? 'The one host is online' : `All ${total} are online`
  if (online === 0) return total === 1 ? 'The one host is offline' : `None of the ${total} is online`
  return `${online} of ${total} are online`
}
