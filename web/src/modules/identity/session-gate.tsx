import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react'
import { ApiError, onUnauthenticated } from '@/kit/api'
import { SessionContext, type Principal } from '@/kit/session'
import { Loading, Problem } from '@/kit/ui'
import { identityApi } from './api'
import { EntryFrame, SetupScreen, SignInScreen } from './entry-screens'

type Phase =
  | { at: 'checking' }
  | { at: 'unreachable'; error: unknown }
  | { at: 'setup' }
  | { at: 'signed-out'; expired: boolean }
  | { at: 'signed-in'; principal: Principal }

async function findPhase(): Promise<Phase> {
  if (await identityApi.setupNeeded()) return { at: 'setup' }
  try {
    const user = await identityApi.currentUser()
    return { at: 'signed-in', principal: user }
  } catch (error) {
    if (error instanceof ApiError && error.code === 'unauthenticated') return { at: 'signed-out', expired: false }
    throw error
  }
}

/** Stands in front of the app: first-run setup when the server has no
 *  account, sign-in when nobody is signed in, the app otherwise. The address
 *  bar is left alone, so signing in lands on the page that was asked for. */
export function SessionGate({ children }: { children: ReactNode }) {
  const [phase, setPhase] = useState<Phase>({ at: 'checking' })

  const check = useCallback(() => {
    setPhase({ at: 'checking' })
    findPhase().then(setPhase, (error: unknown) => setPhase({ at: 'unreachable', error }))
  }, [])

  useEffect(check, [check])

  // a session that ends while the app is open (expiry, removal of the user)
  useEffect(
    () => onUnauthenticated(() => setPhase((p) => (p.at === 'signed-in' ? { at: 'signed-out', expired: true } : p))),
    [],
  )

  const principal = phase.at === 'signed-in' ? phase.principal : null
  const session = useMemo(
    () =>
      principal && {
        principal,
        signOut: async () => {
          await identityApi.signOut()
          setPhase({ at: 'signed-out', expired: false })
        },
      },
    [principal],
  )

  switch (phase.at) {
    case 'checking':
      return (
        <EntryFrame>
          <Loading what="tendhost" />
        </EntryFrame>
      )
    case 'unreachable':
      return (
        <EntryFrame>
          <Problem error={phase.error} retry={check} />
        </EntryFrame>
      )
    case 'setup':
      return <SetupScreen onDone={(user) => setPhase({ at: 'signed-in', principal: user })} />
    case 'signed-out':
      return (
        <SignInScreen expired={phase.expired} onDone={(user) => setPhase({ at: 'signed-in', principal: user })} />
      )
    case 'signed-in':
      return <SessionContext.Provider value={session}>{children}</SessionContext.Provider>
  }
}
