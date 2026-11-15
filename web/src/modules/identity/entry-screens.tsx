import { useState, type FormEvent, type ReactNode } from 'react'
import { ApiError } from '@/kit/api'
import { Button, Field, Input, Notice, Problem } from '@/kit/ui'
import { useAction } from '@/kit/use-action'
import { identityApi, type User } from './api'
import { MIN_PASSWORD_CHARS, passwordProblem } from './rules'

/** The frame of the screens shown before anyone is signed in. */
export function EntryFrame({ children }: { children: ReactNode }) {
  return (
    <main className="mx-auto flex min-h-dvh max-w-[30rem] flex-col justify-center px-5 py-12">
      <p className="mb-8 text-[1.1rem] font-semibold tracking-[-0.01em]">tendhost</p>
      {children}
    </main>
  )
}

export function SetupScreen({ onDone }: { onDone: (user: User) => void }) {
  const [token, setToken] = useState('')
  const [name, setName] = useState('')
  const [password, setPassword] = useState('')
  const [confirm, setConfirm] = useState('')
  const [checked, setChecked] = useState(false)
  const local = passwordProblem(password, confirm)

  const action = useAction(async () => {
    onDone(await identityApi.completeSetup(token, name, password))
  }, ['setup_token', 'name', 'password'])

  const submit = (event: FormEvent) => {
    event.preventDefault()
    setChecked(true)
    if (local.password || local.confirm) return
    void action.run()
  }

  return (
    <EntryFrame>
      <h1 className="text-[1.75rem] leading-tight font-semibold tracking-[-0.01em]">Create the owner account</h1>
      <p className="mt-2 text-muted">
        This server has no account yet. Whoever can read its output creates the first one, and that account owns
        the server.
      </p>
      <form onSubmit={submit} className="mt-8 space-y-5" noValidate>
        <Field
          label="Setup token"
          hint="The server printed it when it started, on the terminal or in its log. It begins with thsetup_"
          error={action.fieldError('setup_token')}
        >
          {(control) => (
            <Input
              {...control}
              value={token}
              onChange={(e) => setToken(e.target.value)}
              className="font-mono text-[0.87rem]"
              autoComplete="off"
              spellCheck={false}
              required
              autoFocus
            />
          )}
        </Field>
        <Field label="Name" hint="What you sign in with" error={action.fieldError('name')}>
          {(control) => (
            <Input
              {...control}
              value={name}
              onChange={(e) => setName(e.target.value)}
              autoComplete="username"
              autoCapitalize="none"
              required
            />
          )}
        </Field>
        <Field
          label="Password"
          hint={`At least ${MIN_PASSWORD_CHARS} characters`}
          error={(checked ? local.password : null) ?? action.fieldError('password')}
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
        {action.error ? <Problem error={action.error} /> : null}
        <Button type="submit" tone="primary" disabled={action.busy}>
          {action.busy ? 'Creating the account' : 'Create the owner account'}
        </Button>
      </form>
    </EntryFrame>
  )
}

export function SignInScreen({ expired, onDone }: { expired: boolean; onDone: (user: User) => void }) {
  const [name, setName] = useState('')
  const [password, setPassword] = useState('')
  const action = useAction(async () => {
    onDone(await identityApi.signIn(name, password))
  })
  const wrong = action.error instanceof ApiError && action.error.code === 'unauthenticated'

  return (
    <EntryFrame>
      <h1 className="text-[1.75rem] leading-tight font-semibold tracking-[-0.01em]">Sign in</h1>
      {expired ? (
        <div className="mt-4">
          <Notice tone="quiet">
            <p>Your session ended. Sign in again to continue where you were.</p>
          </Notice>
        </div>
      ) : null}
      <form
        onSubmit={(event) => {
          event.preventDefault()
          void action.run()
        }}
        className="mt-8 space-y-5"
      >
        <Field label="Name">
          {(control) => (
            <Input
              {...control}
              value={name}
              onChange={(e) => setName(e.target.value)}
              autoComplete="username"
              autoCapitalize="none"
              required
              autoFocus
            />
          )}
        </Field>
        <Field label="Password">
          {(control) => (
            <Input
              {...control}
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="current-password"
              required
            />
          )}
        </Field>
        {wrong ? (
          <Notice tone="down">
            <p>That name and password do not match an account.</p>
          </Notice>
        ) : action.error ? (
          <Problem error={action.error} />
        ) : null}
        <Button type="submit" tone="primary" disabled={action.busy}>
          {action.busy ? 'Signing in' : 'Sign in'}
        </Button>
      </form>
    </EntryFrame>
  )
}
