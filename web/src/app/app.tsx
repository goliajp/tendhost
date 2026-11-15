import type { ReactNode } from 'react'
import { BrowserRouter, Link, Navigate, Route, Routes } from 'react-router'
import { SlotProvider, type RouteDef } from '@/kit/module'
import { useCan } from '@/kit/session'
import { Forbidden, Notice, PageHeader } from '@/kit/ui'
import { SessionGate } from '@/modules/identity'
import { HOME, allRoutes, modules } from './registry'
import { Shell } from './shell'

function Guarded({ route }: { route: RouteDef }) {
  const allowed = useCan(route.minRole ?? 'viewer')
  return allowed ? route.element : <Forbidden />
}

function NotFound() {
  return (
    <>
      <PageHeader title="No such page" />
      <Notice tone="quiet">
        <p>
          Nothing lives at this address.{' '}
          <Link to={HOME} className="text-ink underline underline-offset-4">
            Go to the hosts
          </Link>
        </p>
      </Notice>
    </>
  )
}

/** Each module's provider around the signed-in app, outermost first. */
function Providers({ children }: { children: ReactNode }) {
  return modules.reduceRight<ReactNode>(
    (inner, module) => (module.Provider ? <module.Provider>{inner}</module.Provider> : inner),
    children,
  )
}

export function App() {
  return (
    <BrowserRouter>
      <SessionGate>
        <SlotProvider modules={modules}>
          <Providers>
            <Shell>
              <Routes>
                <Route path="/" element={<Navigate to={HOME} replace />} />
                {allRoutes(modules).map((route) => (
                  <Route key={route.path} path={route.path} element={<Guarded route={route} />} />
                ))}
                <Route path="*" element={<NotFound />} />
              </Routes>
            </Shell>
          </Providers>
        </SlotProvider>
      </SessionGate>
    </BrowserRouter>
  )
}
