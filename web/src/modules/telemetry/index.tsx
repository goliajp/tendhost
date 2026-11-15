import type { ModuleDef } from '@/kit/module'
import { HostCharts } from './host-charts'
import { HostState, HostsSummary, PresenceProvider } from './presence'

/** No page of its own: it fills the places the hosts pages leave open. */
export const telemetry: ModuleDef = {
  id: 'telemetry',
  routes: [],
  nav: [],
  Provider: PresenceProvider,
  fills: {
    'host.state': HostState,
    'host.sections': HostCharts,
    'hosts.summary': HostsSummary,
  },
}
