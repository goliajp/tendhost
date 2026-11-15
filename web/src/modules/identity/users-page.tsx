import { useState, type FormEvent } from 'react'
import { ConfirmButton } from '@/kit/confirm'
import { dateTime, parseTime } from '@/kit/format'
import { atLeast, useSession, type Role } from '@/kit/session'
import { Button, Field, Input, Loading, PageHeader, Problem, Rows, Section, Select } from '@/kit/ui'
import { useAction } from '@/kit/use-action'
import { useResource } from '@/kit/use-resource'
import { identityApi, type User } from './api'
import { MIN_PASSWORD_CHARS, ROLE_MEANING, grantable, passwordProblem } from './rules'

export function UsersPage() {
  const { principal } = useSession()
  const users = useResource('users', identityApi.users)
  const remove = useAction(async (id: string) => {
    await identityApi.removeUser(id)
    await users.reload()
  })

  /** an owner is removed only by an owner; nobody removes themselves */
  const removable = (user: User) => user.id !== principal.id && atLeast(principal.role, user.role)

  return (
    <>
      <PageHeader title="Users" lead="The people who can sign in to this server, and what each may do" />
      {users.error ? <Problem error={users.error} retry={users.reload} /> : null}
      {remove.error ? <Problem error={remove.error} /> : null}
      {users.data ? (
        <Rows>
          {users.data.map((user) => (
            <li key={user.id} className="flex flex-wrap items-center justify-between gap-x-6 gap-y-2 py-3">
              <div className="min-w-0">
                <p className="font-medium">
                  {user.name}
                  {user.id === principal.id ? <span className="font-normal text-muted"> (you)</span> : null}
                </p>
                <p className="text-[0.87rem] text-muted">
                  {user.role}, since {dateTime(parseTime(user.created_at))}
                </p>
              </div>
              {removable(user) ? (
                <ConfirmButton
                  label="Remove"
                  question={`Remove ${user.name}, their sessions and their tokens?`}
                  confirmLabel={`Remove ${user.name}`}
                  busy={remove.busy}
                  onConfirm={() => void remove.run(user.id)}
                />
              ) : null}
            </li>
          ))}
        </Rows>
      ) : users.loading ? (
        <Loading what="users" />
      ) : null}
      <Section title="Add a user">
        <NewUserForm maker={principal.role} onCreated={users.reload} />
      </Section>
    </>
  )
}

function NewUserForm({ maker, onCreated }: { maker: Role; onCreated: () => Promise<void> }) {
  const [name, setName] = useState('')
  const [password, setPassword] = useState('')
  const [confirm, setConfirm] = useState('')
  const [role, setRole] = useState<Role>('viewer')
  const [checked, setChecked] = useState(false)
  const local = passwordProblem(password, confirm)

  const create = useAction(async () => {
    await identityApi.createUser(name, password, role)
    setName('')
    setPassword('')
    setConfirm('')
    setChecked(false)
    await onCreated()
  }, ['name', 'password'])

  const submit = (event: FormEvent) => {
    event.preventDefault()
    setChecked(true)
    if (local.password || local.confirm) return
    void create.run()
  }

  return (
    <form onSubmit={submit} className="max-w-[30rem] space-y-5" noValidate>
      <Field label="Name" error={create.fieldError('name')}>
        {(control) => (
          <Input
            {...control}
            value={name}
            onChange={(e) => setName(e.target.value)}
            autoComplete="off"
            autoCapitalize="none"
            required
          />
        )}
      </Field>
      <Field
        label="Password"
        hint={`At least ${MIN_PASSWORD_CHARS} characters. Pass it to them yourself; the server sends no mail`}
        error={(checked ? local.password : null) ?? create.fieldError('password')}
      >
        {(control) => (
          <Input
            {...control}
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoComplete="new-password"
            required
          />
        )}
      </Field>
      <Field label="Password again" error={checked ? local.confirm : null}>
        {(control) => (
          <Input
            {...control}
            type="password"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
            autoComplete="new-password"
            required
          />
        )}
      </Field>
      <Field label="Role" hint={ROLE_MEANING[role]}>
        {(control) => (
          <Select {...control} value={role} onChange={(e) => setRole(e.target.value as Role)}>
            {grantable(maker).map((r) => (
              <option key={r} value={r}>
                {r}
              </option>
            ))}
          </Select>
        )}
      </Field>
      {create.error ? <Problem error={create.error} /> : null}
      <Button type="submit" tone="primary" disabled={create.busy}>
        {create.busy ? 'Adding the user' : 'Add the user'}
      </Button>
    </form>
  )
}
