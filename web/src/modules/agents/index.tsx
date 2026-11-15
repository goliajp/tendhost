import type { ModuleDef } from '@/kit/module'
import { AgentsPage } from './agents-page'
import { TrackPage } from './track-page'

export const agents: ModuleDef = {
  id: 'agents',
  routes: [
    { path: '/agents', element: <AgentsPage /> },
    { path: '/agents/track', element: <TrackPage />, minRole: 'admin' },
  ],
  nav: [{ to: '/agents', label: 'Coding agents', order: 30 }],
}
