import type { ModuleDef } from '@/kit/module'
import { AuditPage } from './audit-page'

export const audit: ModuleDef = {
  id: 'audit',
  routes: [{ path: '/audit', element: <AuditPage />, minRole: 'admin' }],
  nav: [{ to: '/audit', label: 'Audit log', order: 90, minRole: 'admin' }],
}
