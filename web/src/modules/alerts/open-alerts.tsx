import { createContext, useContext, type ReactNode } from 'react'
import { Link } from 'react-router'
import { dateTime, parseTime } from '@/kit/format'
import { Notice } from '@/kit/ui'
import { useResource, type Resource } from '@/kit/use-resource'
import { alertsApi, type Alert } from './api'

const REFRESH_MS = 15_000

const OpenContext = createContext<Resource<Alert[]> | null>(null)

/** One poll of the open alerts for the navigation, the host pages and the
 *  alerts page together. */
export function OpenAlertsProvider({ children }: { children: ReactNode }) {
  const open = useResource('alerts:open', alertsApi.open, REFRESH_MS)
  return <OpenContext.Provider value={open}>{children}</OpenContext.Provider>
}

export function useOpenAlerts(): Resource<Alert[]> {
  const open = useContext(OpenContext)
  if (!open) throw new Error('useOpenAlerts outside its provider')
  return open
}

/** After the navigation entry: how many alerts are open; nothing when none. */
export function OpenCount() {
  const count = useOpenAlerts().data?.length ?? 0
  if (count === 0) return null
  return (
    <span
      aria-label={`${count} open`}
      className="tabular ml-2 inline-block min-w-[1.4rem] rounded-full bg-down px-1.5 text-center text-[0.8rem] font-medium text-surface"
    >
      {count}
    </span>
  )
}

/** Fills `host.notices`: the alerts open for this host. */
export function HostAlerts({ hostId }: { hostId: string }) {
  const alerts = useOpenAlerts().data?.filter((alert) => alert.host_id === hostId) ?? []
  if (alerts.length === 0) return null
  return (
    <div className="mb-8 space-y-2">
      {alerts.map((alert) => (
        <Notice key={alert.id} tone="down">
          <p className="font-medium">{alert.summary}</p>
          <p className="text-[0.93rem]">
            {alert.detail} Open since {dateTime(parseTime(alert.opened_at))}.{' '}
            <Link to="/alerts" className="text-ink underline underline-offset-4">
              All alerts
            </Link>
          </p>
        </Notice>
      ))}
    </div>
  )
}
