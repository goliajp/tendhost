import { useCallback, useEffect, useState } from 'react'
import { Link } from 'react-router'
import { countdown, parseTime } from '@/kit/format'
import { Button, Copyable, LinkButton, Notice, PageHeader, Problem, WizardStep as Step } from '@/kit/ui'
import { useAction } from '@/kit/use-action'
import { useNow } from '@/kit/use-resource'
import { hostsApi, type Host, type Mode, type NewEnrollment } from './api'
import {
  MODE_MEANING,
  MODE_NAME,
  RUN_COMMAND,
  SERVICE_COMMAND,
  admittedBy,
  enrollCommand,
  installCommand,
  sendsCredentialInClear,
} from './rules'

const POLL_MS = 3000

type Stage =
  | { at: 'choose' }
  | { at: 'commands'; enrollment: NewEnrollment }
  | { at: 'joined'; host: Host }

export function AddHostPage() {
  const [mode, setMode] = useState<Mode>('observe')
  const [stage, setStage] = useState<Stage>({ at: 'choose' })
  // stable, so the wait for the machine is not restarted by every render
  const onJoined = useCallback((host: Host) => setStage({ at: 'joined', host }), [])

  const open = useAction(async () => {
    setStage({ at: 'commands', enrollment: await hostsApi.openEnrollment() })
  })

  return (
    <>
      <PageHeader
        title="Add a host"
        lead="Three steps: decide what this server may do on the machine, run one command there, and watch it arrive"
      />
      <ol className="m-0 max-w-[46rem] list-none space-y-10 p-0">
        <Step n={1} title="Decide what this server may do on the machine" active={stage.at === 'choose'}>
          {stage.at === 'choose' ? (
            <>
              <fieldset className="m-0 space-y-3 border-0 p-0">
                <legend className="sr-only">Mode</legend>
                {(['observe', 'manage'] as const).map((option) => (
                  <label
                    key={option}
                    className={`flex cursor-pointer gap-3 rounded-[5px] border px-4 py-3 ${
                      mode === option ? 'border-live bg-surface' : 'border-rule'
                    }`}
                  >
                    <input
                      type="radio"
                      name="mode"
                      className="mt-1.5 accent-(--live)"
                      checked={mode === option}
                      onChange={() => setMode(option)}
                    />
                    <span>
                      <span className="block font-medium">{MODE_NAME[option]}</span>
                      <span className="block text-[0.93rem] text-muted">{MODE_MEANING[option]}</span>
                    </span>
                  </label>
                ))}
              </fieldset>
              <p className="mt-3 max-w-[65ch] text-[0.93rem] text-muted">
                The choice is stored on the machine, in its own configuration file. This server cannot change it
                later; changing it means editing that file there.
              </p>
              {open.error ? (
                <div className="mt-4">
                  <Problem error={open.error} />
                </div>
              ) : null}
              <Button tone="primary" className="mt-4" disabled={open.busy} onClick={() => void open.run()}>
                {open.busy ? 'Getting the commands' : 'Get the commands'}
              </Button>
            </>
          ) : (
            <p className="text-muted">{MODE_NAME[mode]}</p>
          )}
        </Step>

        <Step n={2} title="Run one command on the machine" active={stage.at === 'commands'}>
          {stage.at === 'commands' ? (
            <Commands
              mode={mode}
              enrollment={stage.enrollment}
              onJoined={onJoined}
              onRestart={() => setStage({ at: 'choose' })}
            />
          ) : stage.at === 'joined' ? (
            <p className="text-muted">Done</p>
          ) : null}
        </Step>

        <Step n={3} title="The machine arrives" active={stage.at === 'joined'}>
          {stage.at === 'joined' ? (
            <>
              <Notice tone="live">
                <p className="font-medium">{stage.host.name} joined</p>
                <p className="mt-1">
                  {stage.host.facts.os} {stage.host.facts.os_version}, {stage.host.facts.arch}. It reports for
                  as long as the daemon runs there.
                </p>
              </Notice>
              <div className="mt-4 flex flex-wrap gap-2">
                <LinkButton to={`/hosts/${stage.host.id}`} tone="primary">
                  Open {stage.host.name}
                </LinkButton>
                <Button onClick={() => setStage({ at: 'choose' })}>Add another host</Button>
              </div>
            </>
          ) : null}
        </Step>
      </ol>
      <p className="mt-10">
        <Link to="/hosts" className="text-ink underline decoration-rule underline-offset-4 hover:decoration-ink">
          Back to hosts
        </Link>
      </p>
    </>
  )
}

function Commands({
  mode,
  enrollment,
  onJoined,
  onRestart,
}: {
  mode: Mode
  enrollment: NewEnrollment
  onJoined: (host: Host) => void
  onRestart: () => void
}) {
  const now = useNow()
  const expires = parseTime(enrollment.expires_at)
  const expired = expires !== null && expires.getTime() <= now.getTime()
  const [pollError, setPollError] = useState<unknown>(undefined)

  useEffect(() => {
    if (expired) return
    let stopped = false
    const look = async () => {
      try {
        const joined = admittedBy(enrollment.id, await hostsApi.list())
        if (stopped) return
        setPollError(undefined)
        if (joined) onJoined(joined)
      } catch (error) {
        if (!stopped) setPollError(error)
      }
    }
    const timer = setInterval(() => void look(), POLL_MS)
    return () => {
      stopped = true
      clearInterval(timer)
    }
  }, [expired, enrollment.id, onJoined])

  if (expired) {
    return (
      <>
        <Notice tone="down">
          <p>This enrollment token expired before a machine used it.</p>
        </Notice>
        <Button tone="primary" className="mt-4" onClick={onRestart}>
          Start again with a new token
        </Button>
      </>
    )
  }

  const origin = window.location.origin
  return (
    <div className="space-y-5">
      {sendsCredentialInClear(origin) ? (
        <Notice tone="down">
          <p>
            You opened this page over plain http. A host enrolled with this address sends its credential
            unencrypted on every report. Put the server behind https and open this page there first.
          </p>
        </Notice>
      ) : null}
      <div>
        <p className="mb-2">Run this on the machine, as root or as the user the daemon should run as:</p>
        <Copyable value={installCommand(origin, enrollment.token, mode)} label="Command that installs and joins" />
        <p className="mt-2 max-w-[65ch] text-[0.93rem] text-muted">
          It downloads <code className="font-mono text-[0.87rem]">tendhostd</code> for the machine, joins this
          server and keeps the daemon running as a systemd or launchd service. The token in it admits one machine
          and stops working in{' '}
          <span className="tabular font-medium text-ink">{expires ? countdown(expires, now) : '—'}</span>.
        </p>
      </div>
      <details className="rounded-[5px] border border-rule px-4 py-3">
        <summary className="cursor-pointer font-medium">The machine already has tendhostd</summary>
        <div className="mt-3 space-y-4">
          <div>
            <p className="mb-2">Join the server:</p>
            <Copyable value={enrollCommand(origin, enrollment.token, mode)} label="Command that joins the server" />
            <p className="mt-2 text-[0.93rem] text-muted">
              Add <code className="font-mono text-[0.87rem]">--name</code> to choose the name it appears under; the
              default is its hostname.
            </p>
          </div>
          <div>
            <p className="mb-2">Then keep it reporting, as a service that survives restarts:</p>
            <Copyable value={SERVICE_COMMAND} label="Command that installs the service" />
            <p className="mt-2 text-[0.93rem] text-muted">
              Or report in the foreground until stopped, with{' '}
              <code className="font-mono text-[0.87rem]">{RUN_COMMAND}</code>.
            </p>
          </div>
        </div>
      </details>
      <div className="border-t border-rule pt-4">
        <p role="status" className="text-muted">
          Waiting for the machine. This page looks every few seconds and moves on by itself when it joins
        </p>
        {pollError ? (
          <div className="mt-3">
            <Problem error={pollError} />
          </div>
        ) : null}
      </div>
    </div>
  )
}
