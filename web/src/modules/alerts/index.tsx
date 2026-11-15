import type { ModuleDef } from '@/kit/module'
import { AlertsPage } from './alerts-page'
import { HostAlerts, OpenAlertsProvider, OpenCount } from './open-alerts'

export const alerts: ModuleDef = {
  id: 'alerts',
  routes: [{ path: '/alerts', element: <AlertsPage /> }],
  nav: [{ to: '/alerts', label: 'Alerts', order: 40, Count: OpenCount }],
  Provider: OpenAlertsProvider,
  fills: { 'host.notices': HostAlerts },
}
