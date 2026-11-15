import type { ModuleDef } from '@/kit/module'
import { TokensPage } from './tokens-page'
import { UsersPage } from './users-page'

export { SessionGate } from './session-gate'

export const identity: ModuleDef = {
  id: 'identity',
  routes: [
    { path: '/users', element: <UsersPage />, minRole: 'admin' },
    { path: '/tokens', element: <TokensPage />, minRole: 'admin' },
  ],
  nav: [
    { to: '/users', label: 'Users', order: 80, minRole: 'admin' },
    { to: '/tokens', label: 'API tokens', order: 81, minRole: 'admin' },
  ],
}
