import { useState, type FormEvent } from 'react'
import { Link, useNavigate, useSearchParams } from 'react-router'
import { Button, Field, Input, LinkButton, Loading, Notice, PageHeader, Problem, Select, Textarea } from '@/kit/ui'
import { useAction } from '@/kit/use-action'
import { useResource } from '@/kit/use-resource'
import { jobsApi, type Schedule } from './api'
import { cronInWords, cronProblem, localInputToIso, span } from './rules'

type Kind = Schedule['kind']

const KIND_NAME: Record<Kind, string> = {
  manual: 'Only when someone runs it',
  once: 'Once, at a time',
  cron: 'Repeatedly, by a cron expression',
}

export function NewJobPage() {
  const navigate = useNavigate()
  const [params] = useSearchParams()
  const hosts = useResource('jobs:hosts', jobsApi.hosts)
  const runnable = hosts.data?.filter((host) => host.mode === 'manage')

  const [name, setName] = useState('')
  const [hostId, setHostId] = useState(params.get('host') ?? '')
  const [command, setCommand] = useState('')
  const [timeout, setTimeoutSecs] = useState('3600')
  const [kind, setKind] = useState<Kind>('manual')
  const [at, setAt] = useState('')
  const [expr, setExpr] = useState('0 3 * * *')
  const [localError, setLocalError] = useState<string | null>(null)

  const chosenHost = runnable?.some((host) => host.id === hostId) ? hostId : (runnable?.[0]?.id ?? '')
  const problem = cronProblem(expr)
  const preview = problem ? null : cronInWords(expr)

  const create = useAction(async (schedule: Schedule) => {
    const job = await jobsApi.create({
      name,
      host_id: chosenHost,
      command,
      timeout_secs: Number(timeout),
      schedule,
    })
    await navigate(`/jobs/${job.id}`)
  }, ['name', 'command', 'timeout_secs', 'schedule'])

  const submit = (event: FormEvent) => {
    event.preventDefault()
    setLocalError(null)
    if (kind === 'once') {
      const iso = localInputToIso(at)
      if (!iso) return setLocalError('Choose the date and time')
      return void create.run({ kind: 'once', at: iso })
    }
    void create.run(kind === 'cron' ? { kind: 'cron', expr: expr.trim() } : { kind: 'manual' })
  }

  return (
    <>
      <p className="mb-2 text-[0.93rem]">
        <Link to="/jobs" className="text-muted underline decoration-rule underline-offset-4 hover:text-ink">
          Jobs
        </Link>
      </p>
      <PageHeader title="Add a job" lead="One shell command on one host, run by the daemon there as the user it runs as" />
      {hosts.error ? <Problem error={hosts.error} retry={hosts.reload} /> : null}
      {runnable === undefined ? (
        hosts.loading ? (
          <Loading what="hosts" />
        ) : null
      ) : runnable.length === 0 ? (
        <NoRunnableHost any={(hosts.data?.length ?? 0) > 0} />
      ) : (
        <form onSubmit={submit} className="max-w-[40rem] space-y-5" noValidate>
          <Field label="Name" hint="What it does, such as nightly backup" error={create.fieldError('name')}>
            {(control) => (
              <Input {...control} value={name} onChange={(e) => setName(e.target.value)} autoComplete="off" required />
            )}
          </Field>
          <Field
            label="Host"
            hint="Only hosts enrolled allowing commands are listed. A host that only reports cannot be given a job from here"
          >
            {(control) => (
              <Select {...control} value={chosenHost} onChange={(e) => setHostId(e.target.value)}>
                {runnable.map((host) => (
                  <option key={host.id} value={host.id}>
                    {host.name}
                  </option>
                ))}
              </Select>
            )}
          </Field>
          <Field
            label="Command"
            hint="Run with sh -c. Its output is kept, the last 64 KiB of it"
            error={create.fieldError('command')}
          >
            {(control) => (
              <Textarea
                {...control}
                value={command}
                onChange={(e) => setCommand(e.target.value)}
                rows={3}
                spellCheck={false}
                className="font-mono text-[0.87rem]"
                required
              />
            )}
          </Field>
          <Field
            label="Time limit, in seconds"
            hint={
              Number(timeout) > 0
                ? `The host stops the command after ${span(Number(timeout))}. At most 24 h`
                : 'The host stops the command after this long. At most 24 h'
            }
            error={create.fieldError('timeout_secs')}
          >
            {(control) => (
              <Input
                {...control}
                type="number"
                min={1}
                max={86400}
                value={timeout}
                onChange={(e) => setTimeoutSecs(e.target.value)}
                className="max-w-[10rem]"
              />
            )}
          </Field>

          <fieldset className="mx-0 space-y-2 border-0 p-0">
            <legend className="mb-1 text-[0.93rem] font-medium">When it runs</legend>
            {(Object.keys(KIND_NAME) as Kind[]).map((option) => (
              <label key={option} className="flex cursor-pointer items-baseline gap-2">
                <input
                  type="radio"
                  name="schedule"
                  className="accent-(--live)"
                  checked={kind === option}
                  onChange={() => setKind(option)}
                />
                {KIND_NAME[option]}
              </label>
            ))}
          </fieldset>

          {kind === 'once' ? (
            <Field label="Date and time" hint="In your time zone" error={localError ?? create.fieldError('schedule')}>
              {(control) => (
                <Input
                  {...control}
                  type="datetime-local"
                  value={at}
                  onChange={(e) => setAt(e.target.value)}
                  className="max-w-[16rem]"
                />
              )}
            </Field>
          ) : null}

          {kind === 'cron' ? (
            <>
              <Field
                label="Cron expression"
                hint="Five fields, separated by spaces: minute (0–59), hour (0–23), day of the month (1–31), month (1–12), day of the week (0–6, Sunday is 0). Use * for every, */5 for every fifth, 1-5 for a range, 1,3 for a list. Times are UTC"
                error={create.fieldError('schedule')}
              >
                {(control) => (
                  <Input
                    {...control}
                    value={expr}
                    onChange={(e) => setExpr(e.target.value)}
                    spellCheck={false}
                    autoComplete="off"
                    className="max-w-[16rem] font-mono text-[0.87rem]"
                  />
                )}
              </Field>
              <p role="status" className="text-[0.93rem]">
                {preview ? (
                  <>
                    <span className="text-muted">That reads: </span>
                    {preview}
                  </>
                ) : (
                  <span className="text-down">{problem}</span>
                )}
              </p>
            </>
          ) : null}

          {create.error ? <Problem error={create.error} /> : null}
          <Button type="submit" tone="primary" disabled={create.busy}>
            {create.busy ? 'Adding the job' : 'Add the job'}
          </Button>
        </form>
      )}
    </>
  )
}

function NoRunnableHost({ any }: { any: boolean }) {
  return (
    <div className="max-w-[65ch] space-y-4">
      <Notice tone="quiet">
        <p className="font-medium">
          {any ? 'None of your hosts lets this server run commands' : 'No host is enrolled yet'}
        </p>
        <p className="mt-1 text-muted">
          A host decides at enrollment whether it only reports or also runs commands, and keeps that choice in its
          own configuration file. This server cannot switch it on. To run jobs on a machine, enroll it choosing
          “Report and run commands”; a machine that is already enrolled has to be removed and enrolled again.
        </p>
      </Notice>
      <LinkButton to="/hosts/add" tone="primary">
        Add a host that allows commands
      </LinkButton>
    </div>
  )
}
