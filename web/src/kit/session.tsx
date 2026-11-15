// Who is signed in. The module that owns sign-in provides it; every other
// module only reads it.

import { createContext, useContext } from 'react'

export type Role = 'viewer' | 'admin' | 'owner'

const RANK: Record<Role, number> = { viewer: 0, admin: 1, owner: 2 }

/** a later role may do everything an earlier one may */
export function atLeast(role: Role, min: Role): boolean {
  return RANK[role] >= RANK[min]
}

export interface Principal {
  id: string
  name: string
  role: Role
}

export interface Session {
  principal: Principal
  signOut: () => Promise<void>
}

export const SessionContext = createContext<Session | null>(null)

export function useSession(): Session {
  const session = useContext(SessionContext)
  if (!session) throw new Error('useSession outside a signed-in tree')
  return session
}

export function useCan(min: Role): boolean {
  return atLeast(useSession().principal.role, min)
}
