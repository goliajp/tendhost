import { useState, type FormEvent, type ReactNode } from 'react'
import { Link } from 'react-router'
import { ago, dateTime, parseTime } from '@/kit/format'
import { useCan } from '@/kit/session'
import { Button, Field, Input, Loading, Notice, PageHeader, Problem, Section, StateTag } from '@/kit/ui'
import { useAction } from '@/kit/use-action'
import { useNow, useResource } from '@/kit/use-resource'
import { alertsApi, type Alert, type Settings } from './api'
import { useOpenAlerts } from './open-alerts'
import { KIND_NAME, fromForm, toForm, type Form } from './rules'

const LINK = 'text-ink underline decoration-rule underline-offset-4 hover:decoration-ink'

export function AlertsPage() {
  const canChange = useCan('admin')
  const open = useOpenAlerts()
  const [showHistory, setShowHistory] = useState(false)

  return (
    <>
      <PageHeader
        title="Alerts"
        lead={
          <>
            An alert opens when a condition below becomes true and resolves by itself when it stops being true.
            {canChange ? (
              <>
                {' '}
                Each opening and resolving is sent over every{' '}
                <Link to="/notifications" className={LINK}>
                  notification channel
                </Link>
              </>
            ) : null}
          </>
        }
      />
      <Section title="Open now" ruled={false}>
        {open.error ? <Problem error={open.error} retry={open.reload} /> : null}
        {open.data === undefined ? (
          open.loading ? (
            <Loading what="alerts" />
          ) : null
        ) : open.data.length === 0 ? (
          <StateTag tone="live">Nothing needs attention</StateTag>
        ) : (
          <AlertList alerts={open.data} />
        )}
      </Section>

      <Section
        title="History"
        ruled={false}
        aside={
          <Button onClick={() => setShowHistory((shown) => !shown)}>{showHistory ? 'Hide' : 'Show the last 200'}</Button>
        }
      >
        {showHistory ? <History /> : <p className="text-muted">Opened and resolved alerts, newest first.</p>}
      </Section>

      <Section title="What raises an alert">
        <SettingsSection canChange={canChange} />
      </Section>
    </>
  )
}

function AlertList({ alerts }: { alerts: Alert[] }) {
  const now = useNow(30_000)
  return (
    <ul className="m-0 list-none divide-y divide-rule border-y border-rule p-0">
      {alerts.map((alert) => {
        const resolved = parseTime(alert.resolved_at)
        return (
          <li key={alert.id} className="grid gap-x-6 gap-y-1 py-3 sm:grid-cols-[1fr_15rem]">
            <div className="min-w-0">
              <p className="font-medium">
                <StateTag tone={resolved ? 'quiet' : 'down'}>{alert.summary}</StateTag>
              </p>
              <p className="text-[0.93rem] text-muted">
                {KIND_NAME[alert.kind] ?? alert.kind}. {alert.detail}{' '}
                <Link to={`/hosts/${alert.host_id}`} className={LINK}>
                  Open the host
                </Link>
              </p>
            </div>
            <p className="tabular text-[0.87rem] text-muted sm:text-right">
              Since {dateTime(parseTime(alert.opened_at))}
              <br />
              {resolved ? `Resolved ${dateTime(resolved)}` : `Open for ${ago(parseTime(alert.opened_at), now).replace(' ago', '')}`}
            </p>
          </li>
        )
      })}
    </ul>
  )
}

function History() {
  const history = useResource('alerts:history', alertsApi.history, 30_000)
  if (history.error) return <Problem error={history.error} retry={history.reload} />
  if (history.data === undefined) return <Loading what="the history" />
  if (history.data.length === 0) return <p className="text-muted">No alert has ever opened.</p>
  return <AlertList alerts={history.data} />
}

function SettingsSection({ canChange }: { canChange: boolean }) {
  const settings = useResource('alerts:settings', alertsApi.settings)
  // kept here: the form below starts afresh from each stored version
  const [saved, setSaved] = useState(false)
  if (settings.error) return <Problem error={settings.error} retry={settings.reload} />
  if (settings.data === undefined) return <Loading what="the settings" />
  return canChange ? (
    <SettingsForm
      key={JSON.stringify(settings.data)}
      settings={settings.data}
      reload={settings.reload}
      saved={saved}
      setSaved={setSaved}
    />
  ) : (
    <ReadOnly settings={settings.data} />
  )
}

const percent = (ratio: number) => `${Math.round(ratio * 100)}%`

function ReadOnly({ settings }: { settings: Settings }) {
  const rows: [string, string][] = [
    ['A host goes offline', settings.host_offline ? 'On' : 'Off'],
    ['A disk fills up', settings.disk_usage === null ? 'Off' : `On, at ${percent(settings.disk_usage)} of its capacity`],
    ['An agent limit is nearly used', settings.agent_limit === null ? 'Off' : `On, at ${percent(settings.agent_limit)} of a window`],
    ['A job fails', settings.job_failed ? 'On' : 'Off'],
  ]
  return (
    <dl className="m-0 grid max-w-[40rem] grid-cols-[minmax(12rem,auto)_1fr] gap-x-6 gap-y-1.5">
      {rows.map(([name, value]) => (
        <div key={name} className="contents">
          <dt className="text-muted">{name}</dt>
          <dd className="m-0">{value}</dd>
        </div>
      ))}
    </dl>
  )
}

function SettingsForm({
  settings,
  reload,
  saved,
  setSaved,
}: {
  settings: Settings
  reload: () => Promise<void>
  saved: boolean
  setSaved: (saved: boolean) => void
}) {
  const [form, setForm] = useState<Form>(() => toForm(settings))
  const [bad, setBad] = useState<'disk_usage' | 'agent_limit' | null>(null)
  const set = (change: Partial<Form>) => {
    setSaved(false)
    setForm((now) => ({ ...now, ...change }))
  }

  const save = useAction(async (next: Settings) => {
    await alertsApi.setSettings(next)
    setSaved(true)
    await reload()
  }, ['disk_usage', 'agent_limit'])

  const submit = (event: FormEvent) => {
    event.preventDefault()
    const parsed = fromForm(form)
    setBad(parsed.ok ? null : parsed.field)
    if (parsed.ok) void save.run(parsed.settings)
  }
  const complaint = 'A whole number from 1 to 100'

  return (
    <form onSubmit={submit} className="max-w-[40rem] space-y-5" noValidate>
      <Toggle
        label="A host goes offline"
        hint="Not heard from for 90 seconds. Resolves when it reports again"
        checked={form.hostOffline}
        onChange={(on) => set({ hostOffline: on })}
      />
      <Toggle
        label="A disk fills up"
        hint="Resolves when the disk is below the level again"
        checked={form.diskOn}
        onChange={(on) => set({ diskOn: on })}
      >
        {form.diskOn ? (
          <Field
            label="Alert at this percentage of a disk’s capacity"
            error={bad === 'disk_usage' ? complaint : save.fieldError('disk_usage')}
          >
            {(control) => (
              <Input
                {...control}
                inputMode="numeric"
                value={form.diskPercent}
                onChange={(e) => set({ diskPercent: e.target.value })}
                className="max-w-[6rem]"
              />
            )}
          </Field>
        ) : null}
      </Toggle>
      <Toggle
        label="A coding agent’s limit is nearly used"
        hint="For tracked agents that keep limit readings. Resolves when a later reading is below the level"
        checked={form.agentOn}
        onChange={(on) => set({ agentOn: on })}
      >
        {form.agentOn ? (
          <Field
            label="Alert at this percentage of a limit window"
            error={bad === 'agent_limit' ? complaint : save.fieldError('agent_limit')}
          >
            {(control) => (
              <Input
                {...control}
                inputMode="numeric"
                value={form.agentPercent}
                onChange={(e) => set({ agentPercent: e.target.value })}
                className="max-w-[6rem]"
              />
            )}
          </Field>
        ) : null}
      </Toggle>
      <Toggle
        label="A job fails"
        hint="A run that failed, timed out, was missed or was lost. Resolves when the job next succeeds"
        checked={form.jobFailed}
        onChange={(on) => set({ jobFailed: on })}
      />
      {save.error ? <Problem error={save.error} /> : null}
      {saved ? (
        <Notice tone="live">
          <p>Saved. Conditions switched off stop raising new alerts.</p>
        </Notice>
      ) : null}
      <Button type="submit" tone="primary" disabled={save.busy}>
        {save.busy ? 'Saving' : 'Save the settings'}
      </Button>
    </form>
  )
}

function Toggle({
  label,
  hint,
  checked,
  onChange,
  children,
}: {
  label: string
  hint: string
  checked: boolean
  onChange: (on: boolean) => void
  children?: ReactNode
}) {
  return (
    <div>
      <label className="flex cursor-pointer items-baseline gap-2.5">
        <input type="checkbox" className="accent-(--live)" checked={checked} onChange={(e) => onChange(e.target.checked)} />
        <span>
          <span className="block font-medium">{label}</span>
          <span className="block text-[0.87rem] text-muted">{hint}</span>
        </span>
      </label>
      {children ? <div className="mt-3 ml-[1.6rem]">{children}</div> : null}
    </div>
  )
}
