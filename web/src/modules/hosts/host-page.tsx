import { useState, type ReactNode } from 'react'
import { Link, useNavigate, useParams } from 'react-router'
import { ApiError } from '@/kit/api'
import { TypedConfirm } from '@/kit/confirm'
import { bytes, count, dateTime, parseTime, text } from '@/kit/format'
import { Slot } from '@/kit/module'
import { useCan } from '@/kit/session'
import { Button, Field, Input, Loading, Notice, PageHeader, Problem, Section } from '@/kit/ui'
import { useAction } from '@/kit/use-action'
import { useResource } from '@/kit/use-resource'
import { hostsApi, type Host } from './api'
import { MODE_NAME, labelToAdd } from './rules'

const REFRESH_MS = 30_000

export function HostPage() {
  const { id = '' } = useParams()
  const host = useResource(`host:${id}`, () => hostsApi.one(id), REFRESH_MS)

  if (host.data === undefined) {
    if (host.error instanceof ApiError && host.error.code === 'not_found') return <Gone />
    if (host.error) return <Problem error={host.error} retry={host.reload} />
    return <Loading what="the host" />
  }
  return <Details host={host.data} reload={host.reload} />
}

function Gone() {
  return (
    <>
      <PageHeader title="No such host" />
      <Notice tone="quiet">
        <p>
          This host is not on the server. It may have been removed.{' '}
          <Link to="/hosts" className="text-ink underline underline-offset-4">
            See the hosts that are
          </Link>
        </p>
      </Notice>
    </>
  )
}

function Details({ host, reload }: { host: Host; reload: () => Promise<void> }) {
  const canChange = useCan('admin')
  return (
    <>
      <p className="mb-2 text-[0.93rem]">
        <Link to="/hosts" className="text-muted underline decoration-rule underline-offset-4 hover:text-ink">
          Hosts
        </Link>
      </p>
      <PageHeader title={host.name} lead={<Slot name="host.state" hostId={host.id} />} />

      <Section title="What it says about itself" aside={<Reported at={host.facts_at} />}>
        <dl className="m-0 grid grid-cols-[minmax(8rem,auto)_1fr] gap-x-6 gap-y-1.5">
          <Fact name="Hostname">{text(host.facts.hostname)}</Fact>
          <Fact name="System">
            {text(host.facts.os)} {host.facts.os_version}
          </Fact>
          <Fact name="Architecture">{text(host.facts.arch)}</Fact>
          <Fact name="CPUs">{count(host.facts.cpu_count)}</Fact>
          <Fact name="Memory">{bytes(host.facts.memory_bytes)}</Fact>
          <Fact name="Daemon">tendhostd {text(host.facts.daemon_version)}</Fact>
          <Fact name="This server may">{MODE_NAME[host.mode].toLowerCase()}</Fact>
          <Fact name="Enrolled">{dateTime(parseTime(host.enrolled_at))}</Fact>
          <Fact name="Id">
            <span className="font-mono text-[0.87rem] break-all">{host.id}</span>
          </Fact>
        </dl>
      </Section>

      <Slot name="host.sections" hostId={host.id} />

      <Section title="Labels">
        <Labels host={host} canChange={canChange} reload={reload} />
      </Section>

      {canChange ? (
        <>
          <Section title="Rename">
            <Rename key={host.name} host={host} reload={reload} />
          </Section>
          <Section title="Remove">
            <Remove host={host} />
          </Section>
        </>
      ) : null}
    </>
  )
}

function Reported({ at }: { at: string }) {
  return <span className="text-[0.87rem] text-muted">reported {dateTime(parseTime(at))}</span>
}

function Fact({ name, children }: { name: string; children: ReactNode }) {
  return (
    <>
      <dt className="text-muted">{name}</dt>
      <dd className="m-0 min-w-0">{children}</dd>
    </>
  )
}

function Labels({ host, canChange, reload }: { host: Host; canChange: boolean; reload: () => Promise<void> }) {
  const [typed, setTyped] = useState('')
  const save = useAction(async (labels: string[]) => {
    await hostsApi.update(host.id, { labels })
    setTyped('')
    await reload()
  }, ['labels'])

  const adding = labelToAdd(typed, host.labels)
  return (
    <div className="max-w-[30rem] space-y-4">
      {host.labels.length === 0 ? (
        <p className="text-muted">
          No labels. A label is a word of your own, such as web or office, to tell hosts apart.
        </p>
      ) : (
        <ul className="m-0 flex list-none flex-wrap gap-2 p-0">
          {host.labels.map((label) => (
            <li key={label} className="inline-flex items-center gap-1 rounded-[5px] border border-rule bg-surface py-0.5 pr-1 pl-2.5">
              {label}
              {canChange ? (
                <button
                  type="button"
                  aria-label={`Remove the label ${label}`}
                  disabled={save.busy}
                  className="rounded-[3px] px-1.5 text-muted hover:text-down disabled:opacity-50"
                  onClick={() => void save.run(host.labels.filter((l) => l !== label))}
                >
                  ×
                </button>
              ) : null}
            </li>
          ))}
        </ul>
      )}
      {canChange ? (
        <form
          className="flex items-end gap-2"
          noValidate
          onSubmit={(event) => {
            event.preventDefault()
            if (adding) void save.run([...host.labels, adding])
          }}
        >
          <div className="min-w-0 flex-1">
            <Field label="New label" error={save.fieldError('labels')}>
              {(control) => (
                <Input {...control} value={typed} onChange={(e) => setTyped(e.target.value)} autoComplete="off" />
              )}
            </Field>
          </div>
          <Button type="submit" disabled={save.busy || adding === null}>
            Add the label
          </Button>
        </form>
      ) : null}
      {save.error ? <Problem error={save.error} /> : null}
    </div>
  )
}

function Rename({ host, reload }: { host: Host; reload: () => Promise<void> }) {
  const [name, setName] = useState(host.name)
  const rename = useAction(async () => {
    await hostsApi.update(host.id, { name })
    await reload()
  }, ['name'])

  return (
    <form
      className="max-w-[30rem] space-y-4"
      noValidate
      onSubmit={(event) => {
        event.preventDefault()
        void rename.run()
      }}
    >
      <Field
        label="Name"
        hint="Lowercase letters, digits, dash, dot and underscore; anything else becomes a dash. The machine's own hostname is not touched"
        error={rename.fieldError('name')}
      >
        {(control) => (
          <Input
            {...control}
            value={name}
            onChange={(e) => setName(e.target.value)}
            autoComplete="off"
            autoCapitalize="none"
            spellCheck={false}
          />
        )}
      </Field>
      {rename.error ? <Problem error={rename.error} /> : null}
      <Button type="submit" disabled={rename.busy || name.trim() === '' || name === host.name}>
        {rename.busy ? 'Renaming' : 'Rename'}
      </Button>
    </form>
  )
}

function Remove({ host }: { host: Host }) {
  const navigate = useNavigate()
  const [asking, setAsking] = useState(false)
  const remove = useAction(async () => {
    await hostsApi.remove(host.id)
    await navigate('/hosts')
  })

  return (
    <div className="max-w-[40rem] space-y-4">
      <p className="text-muted">
        Removing {host.name} deletes its measurements and ends its credential at once; the daemon on the machine
        stops with exit status 78. Nothing on the machine is uninstalled. To bring it back, enroll it again as a
        new host.
      </p>
      {asking ? (
        <TypedConfirm
          expected={host.name}
          prompt={`Type ${host.name} to remove it`}
          confirmLabel={remove.busy ? 'Removing' : `Remove ${host.name}`}
          busy={remove.busy}
          onConfirm={() => void remove.run()}
          onCancel={() => setAsking(false)}
        />
      ) : (
        <Button tone="danger" onClick={() => setAsking(true)}>
          Remove this host
        </Button>
      )}
      {remove.error ? <Problem error={remove.error} /> : null}
    </div>
  )
}
