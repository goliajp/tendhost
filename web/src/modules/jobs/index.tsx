import type { ModuleDef } from '@/kit/module'
import { HostJobs } from './host-jobs'
import { JobPage } from './job-page'
import { JobsPage } from './jobs-page'
import { NewJobPage } from './new-job-page'

export const jobs: ModuleDef = {
  id: 'jobs',
  routes: [
    { path: '/jobs', element: <JobsPage /> },
    { path: '/jobs/new', element: <NewJobPage />, minRole: 'admin' },
    { path: '/jobs/:id', element: <JobPage /> },
  ],
  nav: [{ to: '/jobs', label: 'Jobs', order: 20 }],
  fills: { 'host.sections': HostJobs },
}
