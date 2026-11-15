import { useState, type FormEvent } from 'react'
import { ConfirmButton } from '@/kit/confirm'
import { dateTime, parseTime } from '@/kit/format'
import { Button, Field, Input, Loading, PageHeader, Problem, Rows, Section, Select, StateTag } from '@/kit/ui'
import { useAction } from '@/kit/use-action'
import { useResource } from '@/kit/use-resource'
import { notifyApi, type Channel, type Delivery, type Security } from './api'
import { EMPTY, KIND_MEANING, KIND_NAME, SECURITY, describe, portAfter, toTarget, type Form, type Kind } from './rules'

export function ChannelsPage() {
  const channels = useResource('channels', notifyApi.channels)
  const deliveries = useResource('deliveries', notifyApi.deliveries, 15_000)
  const [tested, setTested] = useState<Record<string, Delivery>>({})

  const test = useAction(async (id: string) => {
    const delivery = await notifyApi.test(id)
    setTested((now) => ({ ...now, [id]: delivery }))
    await deliveries.reload()
  })
  const remove = useAction(async (id: string) => {
    await notifyApi.remove(id)
    await channels.reload()
  })
  const failure = test.error ?? remove.error

  return (
    <>
      <PageHeader
        title="Notifications"
        lead="Where this server sends word when an alert opens or resolves. Every channel gets every alert"
      />
      {channels.error ? <Problem error={channels.error} retry={channels.reload} /> : null}
      {failure ? (
        <div className="mb-6">
          <Problem error={failure} />
        </div>
      ) : null}
      {channels.data === undefined ? (
        channels.loading ? (
          <Loading what="channels" />
        ) : null
      ) : channels.data.length === 0 ? (
        <p className="max-w-[65ch] text-muted">
          No channel exists, so alerts are shown here in the dashboard and sent nowhere. Add one below.
        </p>
      ) : (
        <Rows>
          {channels.data.map((channel) => (
            <ChannelRow
              key={channel.id}
              channel={channel}
              result={tested[channel.id]}
              busy={test.busy || remove.busy}
              onTest={() => void test.run(channel.id)}
              onRemove={() => void remove.run(channel.id)}
            />
          ))}
        </Rows>
      )}

      <Section title="Add a channel">
        <NewChannel onCreated={channels.reload} />
      </Section>

      <Section title="Delivery log" ruled={false} aside={<span className="text-[0.87rem] text-muted">newest first, the last 100</span>}>
        {deliveries.error ? <Problem error={deliveries.error} retry={deliveries.reload} /> : null}
        {deliveries.data === undefined ? (
          deliveries.loading ? (
            <Loading what="deliveries" />
          ) : null
        ) : deliveries.data.length === 0 ? (
          <p className="text-muted">Nothing has been sent yet.</p>
        ) : (
          <Rows>
            {deliveries.data.map((delivery, index) => (
              <li key={index} className="grid gap-x-6 gap-y-1 py-2.5 sm:grid-cols-[11.5rem_1fr]">
                <time dateTime={delivery.at} className="tabular text-[0.93rem] text-muted">
                  {dateTime(parseTime(delivery.at))}
                </time>
                <div className="min-w-0">
                  <DeliveryResult delivery={delivery} />
                  <p className="text-[0.93rem] text-muted">
                    “{delivery.title}” over {delivery.channel_name}
                  </p>
                </div>
              </li>
            ))}
          </Rows>
        )}
      </Section>
    </>
  )
}

function DeliveryResult({ delivery }: { delivery: Delivery }) {
  return delivery.ok ? (
    <StateTag tone="live">Delivered</StateTag>
  ) : (
    <StateTag tone="down">Not delivered: {delivery.error ?? 'the reason was not recorded'}</StateTag>
  )
}

function ChannelRow({
  channel,
  result,
  busy,
  onTest,
  onRemove,
}: {
  channel: Channel
  result: Delivery | undefined
  busy: boolean
  onTest: () => void
  onRemove: () => void
}) {
  return (
    <li className="py-3">
      <div className="flex flex-wrap items-start justify-between gap-x-6 gap-y-2">
        <div className="min-w-0">
          <p className="font-medium">
            {channel.name}
            <span className="font-normal text-muted">, {KIND_NAME[channel.target.kind]}</span>
          </p>
          <p className="text-[0.87rem] break-all text-muted">{describe(channel.target)}</p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button disabled={busy} onClick={onTest}>
            Send a test message
          </Button>
          <ConfirmButton
            label="Remove"
            question={`Remove ${channel.name}? Alerts stop going there`}
            confirmLabel={`Remove ${channel.name}`}
            busy={busy}
            onConfirm={onRemove}
          />
        </div>
      </div>
      {result ? (
        <p role="status" className="mt-2 text-[0.93rem]">
          <span className="text-muted">Test at {dateTime(parseTime(result.at))}: </span>
          <DeliveryResult delivery={result} />
        </p>
      ) : null}
    </li>
  )
}

function NewChannel({ onCreated }: { onCreated: () => Promise<void> }) {
  const [name, setName] = useState('')
  const [form, setForm] = useState<Form>(EMPTY)
  const set = (change: Partial<Form>) => setForm((now) => ({ ...now, ...change }))

  const create = useAction(async () => {
    await notifyApi.create(name, toTarget(form))
    setName('')
    setForm({ ...EMPTY, kind: form.kind })
    await onCreated()
  }, ['name', 'url', 'host', 'port', 'from', 'to'])

  const submit = (event: FormEvent) => {
    event.preventDefault()
    void create.run()
  }
  const text = (field: 'url' | 'token' | 'host' | 'username' | 'from' | 'to') => ({
    value: form[field],
    onChange: (e: { target: { value: string } }) => set({ [field]: e.target.value }),
    autoComplete: 'off',
    spellCheck: false,
  })

  return (
    <form onSubmit={submit} className="max-w-[40rem] space-y-5" noValidate>
      <Field label="Kind" hint={KIND_MEANING[form.kind]}>
        {(control) => (
          <Select {...control} value={form.kind} onChange={(e) => set({ kind: e.target.value as Kind })}>
            {(Object.keys(KIND_NAME) as Kind[]).map((kind) => (
              <option key={kind} value={kind}>
                {KIND_NAME[kind]}
              </option>
            ))}
          </Select>
        )}
      </Field>
      <Field label="Name" hint="How it appears in the delivery log" error={create.fieldError('name')}>
        {(control) => <Input {...control} value={name} onChange={(e) => setName(e.target.value)} autoComplete="off" />}
      </Field>

      {form.kind === 'webhook' ? (
        <Field label="Address" hint="Starts with https:// or http://" error={create.fieldError('url')}>
          {(control) => <Input {...control} {...text('url')} inputMode="url" />}
        </Field>
      ) : null}

      {form.kind === 'ntfy' ? (
        <>
          <Field label="Topic address" hint="Such as https://ntfy.sh/your-topic" error={create.fieldError('url')}>
            {(control) => <Input {...control} {...text('url')} inputMode="url" />}
          </Field>
          <Field label="Access token" hint="Only for a protected topic. It is stored on the server and never shown again">
            {(control) => <Input {...control} {...text('token')} type="password" autoComplete="new-password" />}
          </Field>
        </>
      ) : null}

      {form.kind === 'email' ? (
        <>
          <div className="grid gap-5 sm:grid-cols-[1fr_7rem]">
            <Field label="SMTP server" error={create.fieldError('host')}>
              {(control) => <Input {...control} {...text('host')} />}
            </Field>
            <Field label="Port" error={create.fieldError('port')}>
              {(control) => (
                <Input {...control} inputMode="numeric" value={form.port} onChange={(e) => set({ port: e.target.value })} />
              )}
            </Field>
          </div>
          <Field label="Encryption">
            {(control) => (
              <Select
                {...control}
                value={form.security}
                onChange={(e) => {
                  const next = e.target.value as Security
                  set({ security: next, port: portAfter(form.security, next, form.port) })
                }}
              >
                {(Object.keys(SECURITY) as Security[]).map((security) => (
                  <option key={security} value={security}>
                    {SECURITY[security].name}
                  </option>
                ))}
              </Select>
            )}
          </Field>
          <div className="grid gap-5 sm:grid-cols-2">
            <Field label="User name" hint="Leave empty for a server that takes mail without signing in">
              {(control) => <Input {...control} {...text('username')} />}
            </Field>
            <Field label="Password" hint="Stored on the server and never shown again">
              {(control) => (
                <Input
                  {...control}
                  type="password"
                  value={form.password}
                  onChange={(e) => set({ password: e.target.value })}
                  autoComplete="new-password"
                />
              )}
            </Field>
          </div>
          <div className="grid gap-5 sm:grid-cols-2">
            <Field label="From" error={create.fieldError('from')}>
              {(control) => <Input {...control} {...text('from')} inputMode="email" />}
            </Field>
            <Field label="To" error={create.fieldError('to')}>
              {(control) => <Input {...control} {...text('to')} inputMode="email" />}
            </Field>
          </div>
        </>
      ) : null}

      {create.error ? <Problem error={create.error} /> : null}
      <Button type="submit" tone="primary" disabled={create.busy}>
        {create.busy ? 'Adding the channel' : 'Add the channel'}
      </Button>
      <p className="text-[0.87rem] text-muted">Adding sends nothing. Use “Send a test message” on the channel afterwards.</p>
    </form>
  )
}
