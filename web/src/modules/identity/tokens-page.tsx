import { useState } from 'react'
import { ConfirmButton } from '@/kit/confirm'
import { dateTime, parseTime } from '@/kit/format'
import { useSession, type Role } from '@/kit/session'
import { Button, Copyable, Field, Input, Loading, Notice, PageHeader, Problem, Rows, Section, Select } from '@/kit/ui'
import { useAction } from '@/kit/use-action'
import { useResource } from '@/kit/use-resource'
import { identityApi, type NewToken } from './api'
import { ROLE_MEANING, grantable } from './rules'

export function TokensPage() {
  const { principal } = useSession()
  const tokens = useResource('tokens', identityApi.tokens)
  const [fresh, setFresh] = useState<NewToken | null>(null)
  const [name, setName] = useState('')
  const [role, setRole] = useState<Role>('viewer')

  const create = useAction(async () => {
    setFresh(await identityApi.createToken(name, role))
    setName('')
    await tokens.reload()
  }, ['name'])

  const revoke = useAction(async (id: string) => {
    await identityApi.revokeToken(id)
    setFresh((token) => (token?.id === id ? null : token))
    await tokens.reload()
  })

  return (
    <>
      <PageHeader
        title="API tokens"
        lead="A token lets a script or the command line act on this server, with a role of its own. It acts as the user who made it"
      />
      {fresh ? (
        <div className="mb-8">
          <Notice tone="live">
            <p className="font-medium">Store the secret of “{fresh.name}” now</p>
            <p className="mt-1 mb-3 max-w-[65ch]">
              The server keeps only a fingerprint of it. Once you leave or reload this page it cannot be shown
              again; a lost secret means revoking the token and making a new one.
            </p>
            <Copyable value={fresh.secret} label={`Secret of the token ${fresh.name}`} />
            <Button className="mt-3" onClick={() => setFresh(null)}>
              I have stored it
            </Button>
          </Notice>
        </div>
      ) : null}
      {tokens.error ? <Problem error={tokens.error} retry={tokens.reload} /> : null}
      {revoke.error ? <Problem error={revoke.error} /> : null}
      {tokens.data && tokens.data.length === 0 ? (
        <p className="text-muted">No token exists. Make one below to use the API from a script.</p>
      ) : tokens.data ? (
        <Rows>
          {tokens.data.map((token) => (
            <li key={token.id} className="flex flex-wrap items-center justify-between gap-x-6 gap-y-2 py-3">
              <div className="min-w-0">
                <p className="font-medium">{token.name}</p>
                <p className="text-[0.87rem] text-muted">
                  {token.role}, made {dateTime(parseTime(token.created_at))}
                </p>
              </div>
              <ConfirmButton
                label="Revoke"
                question={`Revoke ${token.name}? Whatever uses it stops working at once`}
                confirmLabel={`Revoke ${token.name}`}
                busy={revoke.busy}
                onConfirm={() => void revoke.run(token.id)}
              />
            </li>
          ))}
        </Rows>
      ) : tokens.loading ? (
        <Loading what="tokens" />
      ) : null}
      <Section title="Make a token">
        <form
          className="max-w-[30rem] space-y-5"
          noValidate
          onSubmit={(event) => {
            event.preventDefault()
            void create.run()
          }}
        >
          <Field label="Name" hint="What it is for, such as backup script" error={create.fieldError('name')}>
            {(control) => (
              <Input {...control} value={name} onChange={(e) => setName(e.target.value)} autoComplete="off" required />
            )}
          </Field>
          <Field label="Role" hint={ROLE_MEANING[role]}>
            {(control) => (
              <Select {...control} value={role} onChange={(e) => setRole(e.target.value as Role)}>
                {grantable(principal.role).map((r) => (
                  <option key={r} value={r}>
                    {r}
                  </option>
                ))}
              </Select>
            )}
          </Field>
          {create.error ? <Problem error={create.error} /> : null}
          <Button type="submit" tone="primary" disabled={create.busy}>
            {create.busy ? 'Making the token' : 'Make the token'}
          </Button>
        </form>
      </Section>
    </>
  )
}
