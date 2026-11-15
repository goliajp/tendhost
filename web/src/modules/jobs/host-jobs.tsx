import { useCan } from '@/kit/session'
import { LinkButton, Problem, Section } from '@/kit/ui'
import { useResource } from '@/kit/use-resource'
import { jobsApi } from './api'
import { JobTable } from './jobs-page'

/** Fills `host.sections`: the jobs that run on this host. */
export function HostJobs({ hostId }: { hostId: string }) {
  const canAdd = useCan('admin')
  const jobs = useResource(`jobs:host:${hostId}`, () => jobsApi.list(hostId), 15_000)
  const hosts = useResource('jobs:hosts', jobsApi.hosts)
  const host = hosts.data?.find((h) => h.id === hostId)
  const runsCommands = host?.mode === 'manage'

  return (
    <Section
      title="Jobs"
      ruled={false}
      aside={
        canAdd && runsCommands && jobs.data && jobs.data.length > 0 ? (
          <LinkButton to={`/jobs/new?host=${encodeURIComponent(hostId)}`}>Add a job here</LinkButton>
        ) : undefined
      }
    >
      {jobs.error ? <Problem error={jobs.error} retry={jobs.reload} /> : null}
      {jobs.data && jobs.data.length > 0 ? (
        <JobTable jobs={jobs.data} hosts={hosts.data ?? []} hideHost />
      ) : jobs.data && host ? (
        runsCommands ? (
          <div className="space-y-3">
            <p className="text-muted">No job runs on this host.</p>
            {canAdd ? <LinkButton to={`/jobs/new?host=${encodeURIComponent(hostId)}`}>Add a job here</LinkButton> : null}
          </div>
        ) : (
          <p className="max-w-[65ch] text-muted">
            This host only reports. It was enrolled without allowing commands, so no job can run on it; that choice
            is kept on the machine and cannot be changed from here.
          </p>
        )
      ) : null}
    </Section>
  )
}
