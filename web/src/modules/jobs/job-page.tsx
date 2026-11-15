import { Link, useNavigate, useParams } from 'react-router'
import { ApiError } from '@/kit/api'
import { ConfirmButton } from '@/kit/confirm'
import { DASH, dateTime, parseTime } from '@/kit/format'
import { useCan } from '@/kit/session'
import { Button, Loading, Notice, PageHeader, Problem, Section, StateTag } from '@/kit/ui'
import { useAction } from '@/kit/use-action'
import { useResource } from '@/kit/use-resource'
import { jobsApi, type Job, type Run } from './api'
import { RUN_STATE, isOver, nextRun, runSeconds, scheduleInWords, span } from './rules'

const LINK = 'text-ink underline decoration-rule underline-offset-4 hover:decoration-ink'

export function JobPage() {
  const { id = '' } = useParams()
  const job = useResource(`job:${id}`, () => jobsApi.one(id), 15_000)
  const hosts = useResource('jobs:hosts', jobsApi.hosts)

  if (job.data === undefined) {
    if (job.error instanceof ApiError && job.error.code === 'not_found') {
      return (
        <>
          <PageHeader title="No such job" />
          <Notice tone="quiet">
            <p>
              This job is not on the server. It may have been removed, or its host was.{' '}
              <Link to="/jobs" className={LINK}>
                See the jobs that are
              </Link>
            </p>
          </Notice>
        </>
      )
    }
    if (job.error) return <Problem error={job.error} retry={job.reload} />
    return <Loading what="the job" />
  }
  const hostName = hosts.data?.find((host) => host.id === job.data?.host_id)?.name ?? null
  return <Details job={job.data} hostName={hostName} reload={job.reload} />
}

function Details({ job, hostName, reload }: { job: Job; hostName: string | null; reload: () => Promise<void> }) {
  const canChange = useCan('admin')
  const navigate = useNavigate()
  // quick while a result can still arrive, slow otherwise
  const runs = useResource(`runs:${job.id}`, () => jobsApi.runs(job.id), 3000)

  const runNow = useAction(async () => {
    await jobsApi.runNow(job.id)
    await runs.reload()
  })
  const toggle = useAction(async () => {
    await (job.enabled ? jobsApi.disable(job.id) : jobsApi.enable(job.id))
    await reload()
  })
  const remove = useAction(async () => {
    await jobsApi.remove(job.id)
    await navigate('/jobs')
  })
  const failure = runNow.error ?? toggle.error ?? remove.error

  return (
    <>
      <p className="mb-2 text-[0.93rem]">
        <Link to="/jobs" className="text-muted underline decoration-rule underline-offset-4 hover:text-ink">
          Jobs
        </Link>
      </p>
      <PageHeader
        title={job.name}
        lead={
          <>
            On{' '}
            <Link to={`/hosts/${job.host_id}`} className={LINK}>
              {hostName ?? 'its host'}
            </Link>
            . {scheduleInWords(job.schedule)}
          </>
        }
        actions={
          canChange ? (
            <>
              <Button tone="primary" disabled={runNow.busy} onClick={() => void runNow.run()}>
                {runNow.busy ? 'Asking the host' : 'Run now'}
              </Button>
              <Button disabled={toggle.busy} onClick={() => void toggle.run()}>
                {job.enabled ? 'Disable' : 'Enable'}
              </Button>
            </>
          ) : undefined
        }
      />
      {failure ? (
        <div className="mb-6">
          <Problem error={failure} />
        </div>
      ) : null}

      <dl className="m-0 grid max-w-[46rem] grid-cols-[minmax(7rem,auto)_1fr] gap-x-6 gap-y-1.5">
        <dt className="text-muted">Command</dt>
        <dd className="m-0 min-w-0">
          <pre className="m-0 overflow-x-auto font-mono text-[0.87rem] whitespace-pre-wrap break-all">{job.command}</pre>
        </dd>
        <dt className="text-muted">Time limit</dt>
        <dd className="m-0">{span(job.timeout_secs)}</dd>
        <dt className="text-muted">Enabled</dt>
        <dd className="m-0">
          <StateTag tone={job.enabled ? 'live' : 'quiet'}>
            {job.enabled ? 'Yes' : 'No: its schedule is ignored, and it can still be run by hand'}
          </StateTag>
        </dd>
        <dt className="text-muted">Next run</dt>
        <dd className="tabular m-0">{nextRun(job)}</dd>
      </dl>

      <Section title="Runs" ruled={false} aside={<span className="text-[0.87rem] text-muted">newest first, the last 50</span>}>
        {runs.error ? <Problem error={runs.error} retry={runs.reload} /> : null}
        {runs.data === undefined ? (
          runs.loading ? (
            <Loading what="runs" />
          ) : null
        ) : runs.data.length === 0 ? (
          <p className="text-muted">
            It has not run yet.{' '}
            {canChange ? 'Run now asks the host to start it at its next report, within 30 seconds.' : ''}
          </p>
        ) : (
          <ol className="m-0 list-none divide-y divide-rule border-y border-rule p-0">
            {runs.data.map((run) => (
              <RunRow key={run.id} run={run} />
            ))}
          </ol>
        )}
      </Section>

      {canChange ? (
        <Section title="Remove">
          <p className="mb-4 max-w-[65ch] text-muted">
            Removing the job deletes its runs and their output. A run already handed to the host is not stopped.
          </p>
          <ConfirmButton
            label="Remove this job"
            question={`Remove ${job.name} and its runs?`}
            confirmLabel={`Remove ${job.name}`}
            busy={remove.busy}
            onConfirm={() => void remove.run()}
          />
        </Section>
      ) : null}
    </>
  )
}

function RunRow({ run }: { run: Run }) {
  const state = RUN_STATE[run.state]
  const seconds = runSeconds(run.started_at, run.finished_at)
  const asked = run.requested_by.kind === 'system' ? 'by the schedule' : 'by hand'
  return (
    <li className="py-3">
      <details className="group">
        <summary className="grid cursor-pointer list-none grid-cols-[1fr] gap-x-6 gap-y-1 sm:grid-cols-[9rem_11.5rem_1fr]">
          <span>
            <span aria-hidden className="mr-1.5 inline-block text-[0.65rem] text-muted transition-transform group-open:rotate-90">
              ▶
            </span>
            <StateTag tone={state.tone}>{state.name}</StateTag>
          </span>
          <time dateTime={run.due_at} className="tabular text-[0.93rem] text-muted">
            {dateTime(parseTime(run.due_at))}
          </time>
          <span className="tabular text-[0.93rem] text-muted">
            asked {asked}
            {run.exit_code !== null ? `, exit status ${run.exit_code}` : ''}
            {seconds !== null ? `, took ${span(seconds)}` : ''}
          </span>
        </summary>
        <div className="mt-3 space-y-2 sm:ml-[1.1rem]">
          <p className="text-[0.93rem]">{state.meaning}</p>
          <p className="tabular text-[0.87rem] text-muted">
            Started {run.started_at ? dateTime(parseTime(run.started_at)) : DASH}, finished{' '}
            {run.finished_at ? dateTime(parseTime(run.finished_at)) : DASH}
          </p>
          {run.output ? (
            <pre
              aria-label="Output"
              className="m-0 max-h-[24rem] overflow-auto rounded-[5px] border border-rule bg-ground px-3 py-2 font-mono text-[0.87rem] leading-relaxed whitespace-pre-wrap break-all"
            >
              {run.output}
            </pre>
          ) : (
            <p className="text-[0.87rem] text-muted">
              {isOver(run.state) ? 'The command printed nothing' : 'Output arrives when the run ends'}
            </p>
          )}
        </div>
      </details>
    </li>
  )
}
