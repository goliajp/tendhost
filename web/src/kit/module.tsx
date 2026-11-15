// What a module hands to the app shell, and how modules extend each other's
// pages without importing each other.

import { createContext, useContext, type ComponentType, type ReactNode } from 'react'
import type { Role } from './session'

export interface RouteDef {
  path: string
  element: ReactNode
  /** below this role the route answers "your role does not allow this" */
  minRole?: Role
}

export interface NavEntry {
  to: string
  label: string
  order: number
  minRole?: Role
  /** rendered after the label: a count of what waits behind the entry */
  Count?: ComponentType
}

/** Places on one module's page that other modules may fill, with the props
 *  the page passes to whatever fills them. */
export interface SlotProps {
  /** a host's row in the hosts list: its state */
  'host.state': { hostId: string }
  /** the host page, directly under its title: what needs attention now */
  'host.notices': { hostId: string }
  /** the host page, below the host's own details */
  'host.sections': { hostId: string }
  /** above the hosts list: a sentence about the hosts as a whole */
  'hosts.summary': { hostIds: string[] }
}

export type SlotName = keyof SlotProps
export type SlotFillers = { [K in SlotName]?: ComponentType<SlotProps[K]> }

export interface ModuleDef {
  id: string
  routes: RouteDef[]
  nav: NavEntry[]
  /** components this module puts into other modules' slots */
  fills?: SlotFillers
  /** wraps the signed-in app; for state a module shares between its own pieces */
  Provider?: ComponentType<{ children: ReactNode }>
}

type Registry = { [K in SlotName]: ComponentType<SlotProps[K]>[] }

export function collectFills(modules: ModuleDef[]): Registry {
  const registry: Registry = { 'host.state': [], 'host.notices': [], 'host.sections': [], 'hosts.summary': [] }
  for (const module of modules) {
    for (const name of Object.keys(module.fills ?? {}) as SlotName[]) {
      const filler = module.fills?.[name]
      if (filler) (registry[name] as ComponentType<never>[]).push(filler as ComponentType<never>)
    }
  }
  return registry
}

const SlotContext = createContext<Registry>(collectFills([]))

export function SlotProvider({ modules, children }: { modules: ModuleDef[]; children: ReactNode }) {
  return <SlotContext.Provider value={collectFills(modules)}>{children}</SlotContext.Provider>
}

/** Renders whatever other modules put into the slot `name`; nothing when
 *  no module fills it. */
export function Slot<K extends SlotName>({ name, ...props }: { name: K } & SlotProps[K]) {
  const fillers = useContext(SlotContext)[name] as ComponentType<SlotProps[K]>[]
  return (
    <>
      {fillers.map((Filler, index) => (
        <Filler key={index} {...(props as unknown as SlotProps[K] & object)} />
      ))}
    </>
  )
}
