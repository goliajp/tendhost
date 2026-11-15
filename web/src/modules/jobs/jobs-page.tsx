import { Link } from 'react-router'
import { useCan } from '@/kit/session'
import { LinkButton, Loading, PageHeader, Problem, StateTag } from '@/kit/ui'
import { useResource } from '@/kit/use-resource'
import { jobsApi, type HostRef, type Job } from './api'
import { RUN_STATE, nextRun, scheduleInWords } from './rules'

const REFRESH_MS = 15_000
const LINK = 'font-medium text-ink underline decoration-rule underline-offset-4 hover:decoration-ink'
const GRID = 'md:grid md:grid-cols-[minmax(9rem,1.2fr)_minmax(7rem,0.8fr)_minmax(12rem,1.6fr)_6rem_11rem] md:gap-x-6'

export function JobsPage() {
  const canAdd = useCan('admin')
  const jobs = useResource('jobs', () => jobsApi.list(), REFRESH_MS)
  const hosts = useResource('jobs:hosts', jobsApi.hosts, REFRESH_MS)
  const error = jobs.error ?? hosts.error

  return (
    <>
      <PageHeader
        title="Jobs"
        lead="Commands this server has a host run, on a schedule or when someone asks"
        actions={
          canAdd && jobs.data && jobs.data.length > 0 ? (
            <LinkButton to="/jobs/new" tone="primary">
              Add a job
            </LinkButton>
          ) : undefined
        }
      />
      {error ? (
        <div className="mb-6">
          <Problem error={error} retry={() => void Promise.all([jobs.reload(), hosts.reload()])} />
        </div>
      ) : null}
      {jobs.data === undefined ? (
        jobs.loading ? (
          <Loading what="jobs" />
        ) : null
      ) : jobs.data.length === 0 ? (
        <div className="max-w-[65ch] border-l-4 border-rule py-1 pl-5">
          <p className="text-[1.1rem] font-medium">No job exists yet</p>
          <p className="mt-1 mb-4 text-muted">
            A job is one shell command on one host: a backup, a cleanup, a check. The host must have been enrolled
            allowing commands.
          </p>
          {canAdd ? (
            <LinkButton to="/jobs/new" tone="primary">
              Add the first job
            </LinkButton>
          ) : (
            <p className="text-muted">An admin or the owner can add one.</p>
          )}
        </div>
      ) : (
        <JobTable jobs={jobs.data} hosts={hosts.data ?? []} />
      )}
    </>
  )
}

export function JobTable({ jobs, hosts, hideHost }: { jobs: Job[]; hosts: HostRef[]; hideHost?: boolean }) {
  const hostName = (id: string) => hosts.find((host) => host.id === id)?.name ?? '—'
  return (
    <div role="table" aria-label="Jobs" className="border-y border-rule">
      <div role="row" className={`${GRID} hidden border-b border-rule py-2 text-[0.87rem] text-muted`}>
        <span role="columnheader">Name</span>
        <span role="columnheader">{hideHost ? '' : 'Host'}</span>
        <span role="columnheader">Schedule</span>
        <span role="columnheader">Last run</span>
        <span role="columnheader">Next run</span>
      </div>
      <div role="rowgroup" className="divide-y divide-rule">
        {jobs.map((job) => (
          <div role="row" key={job.id} className={`${GRID} items-baseline py-3`}>
            <div role="cell" className="min-w-0">
              <Link to={`/jobs/${job.id}`} className={LINK}>
                {job.name}
              </Link>
            </div>
            <div role="cell" className="text-[0.93rem] text-muted">
              {hideHost ? '' : hostName(job.host_id)}
            </div>
            <div role="cell" className="text-[0.93rem]">
              {scheduleInWords(job.schedule)}
            </div>
            <div role="cell" className="text-[0.93rem]">
              {!job.enabled ? (
                <StateTag tone="quiet">Disabled</StateTag>
              ) : job.last_run ? (
                <StateTag tone={RUN_STATE[job.last_run.state].tone}>{RUN_STATE[job.last_run.state].name}</StateTag>
              ) : (
                <span className="text-muted">Not run yet</span>
              )}
            </div>
            <div role="cell" className="tabular text-[0.93rem] text-muted">
              <span className="md:hidden">Next run </span>
              {nextRun(job)}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
