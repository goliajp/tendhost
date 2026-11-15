// Every module of the dashboard. Adding one is a directory under
// src/modules and a line here.

import type { ModuleDef, NavEntry, RouteDef } from '@/kit/module'
import { atLeast, type Role } from '@/kit/session'
import { audit } from '@/modules/audit'
import { hosts } from '@/modules/hosts'
import { identity } from '@/modules/identity'
import { telemetry } from '@/modules/telemetry'

export const modules: ModuleDef[] = [hosts, telemetry, identity, audit]

/** Where `/` leads. */
export const HOME = '/hosts'

export function allRoutes(from: readonly ModuleDef[]): RouteDef[] {
  return from.flatMap((module) => module.routes)
}

/** The navigation a person with `role` sees, in order. */
export function navFor(from: readonly ModuleDef[], role: Role): NavEntry[] {
  return from
    .flatMap((module) => module.nav)
    .filter((entry) => !entry.minRole || atLeast(role, entry.minRole))
    .sort((a, b) => a.order - b.order)
}
