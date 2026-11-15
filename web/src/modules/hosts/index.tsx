import type { ModuleDef } from '@/kit/module'
import { AddHostPage } from './add-host-page'
import { HostPage } from './host-page'
import { HostsPage } from './hosts-page'

export const hosts: ModuleDef = {
  id: 'hosts',
  routes: [
    { path: '/hosts', element: <HostsPage /> },
    { path: '/hosts/add', element: <AddHostPage />, minRole: 'admin' },
    { path: '/hosts/:id', element: <HostPage /> },
  ],
  nav: [{ to: '/hosts', label: 'Hosts', order: 10 }],
}
