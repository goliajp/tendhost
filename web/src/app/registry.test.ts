import { describe, expect, it } from 'vitest'
import type { ModuleDef } from '@/kit/module'
import { HOME, allRoutes, modules, navFor } from './registry'

const fake = (id: string, nav: ModuleDef['nav']): ModuleDef => ({ id, routes: [], nav })

describe('navFor', () => {
  const from = [
    fake('b', [{ to: '/b', label: 'B', order: 20, minRole: 'admin' }]),
    fake('a', [
      { to: '/a', label: 'A', order: 10 },
      { to: '/o', label: 'O', order: 30, minRole: 'owner' },
    ]),
  ]

  it('orders entries across modules and hides what the role may not open', () => {
    expect(navFor(from, 'viewer').map((e) => e.label)).toEqual(['A'])
    expect(navFor(from, 'admin').map((e) => e.label)).toEqual(['A', 'B'])
    expect(navFor(from, 'owner').map((e) => e.label)).toEqual(['A', 'B', 'O'])
  })
})

describe('the real registry', () => {
  it('has no two routes at one path and no two modules with one id', () => {
    const paths = allRoutes(modules).map((route) => route.path)
    expect(new Set(paths).size).toBe(paths.length)
    const ids = modules.map((module) => module.id)
    expect(new Set(ids).size).toBe(ids.length)
  })

  it('leads home to a route that exists and every nav entry to a route', () => {
    const paths = allRoutes(modules).map((route) => route.path)
    expect(paths).toContain(HOME)
    for (const entry of navFor(modules, 'owner')) expect(paths).toContain(entry.to)
  })

  it('guards a nav entry and its route with the same role', () => {
    const routes = allRoutes(modules)
    for (const entry of navFor(modules, 'owner')) {
      expect(routes.find((route) => route.path === entry.to)?.minRole).toBe(entry.minRole)
    }
  })
})
