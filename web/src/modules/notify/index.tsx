import type { ModuleDef } from '@/kit/module'
import { ChannelsPage } from './channels-page'

export const notify: ModuleDef = {
  id: 'notify',
  routes: [{ path: '/notifications', element: <ChannelsPage />, minRole: 'admin' }],
  nav: [{ to: '/notifications', label: 'Notifications', order: 50, minRole: 'admin' }],
}
