import type { ReactNode } from 'react'
import { NavLink } from 'react-router'
import { useSession } from '@/kit/session'
import { Button } from '@/kit/ui'
import { useAction } from '@/kit/use-action'
import { modules, navFor } from './registry'

/** The frame around every signed-in page: a rail on wide screens, a bar
 *  on narrow ones. */
export function Shell({ children }: { children: ReactNode }) {
  const { principal, signOut } = useSession()
  const leave = useAction(signOut)
  const nav = navFor(modules, principal.role)

  return (
    <div className="mx-auto min-h-dvh max-w-[82rem] md:grid md:grid-cols-[13.5rem_1fr]">
      <header className="flex flex-wrap items-center gap-x-5 gap-y-2 border-b border-rule px-5 py-3 md:sticky md:top-0 md:h-dvh md:flex-col md:flex-nowrap md:items-stretch md:gap-0 md:border-r md:border-b-0 md:px-0 md:py-6">
        <p className="m-0 text-[1.1rem] font-semibold tracking-[-0.01em] md:mb-6 md:px-6">tendhost</p>
        <nav aria-label="Sections" className="flex flex-1 flex-wrap gap-x-1 md:flex-none md:flex-col">
          {nav.map((entry) => (
            <NavLink
              key={entry.to}
              to={entry.to}
              className={({ isActive }) =>
                `border-b-2 px-2 py-1 text-ink no-underline md:border-b-0 md:border-l-[3px] md:px-[1.3rem] md:py-1.5 ${
                  isActive ? 'border-live font-medium' : 'border-transparent text-muted hover:text-ink'
                }`
              }
            >
              {entry.label}
            </NavLink>
          ))}
        </nav>
        <div className="flex items-center gap-3 md:mt-auto md:flex-col md:items-start md:gap-2 md:px-6">
          <p className="m-0 text-[0.87rem] text-muted">
            {principal.name}, {principal.role}
          </p>
          <Button disabled={leave.busy} onClick={() => void leave.run()}>
            Sign out
          </Button>
        </div>
      </header>
      <main className="min-w-0 px-5 py-8 md:px-10 md:py-10">{children}</main>
    </div>
  )
}
