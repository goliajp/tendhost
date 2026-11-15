import { describe, expect, it } from 'vitest'
import type { Presence } from './api'
import { standingOf, summary } from './presence'

describe('standingOf', () => {
  const byHost = new Map<string, Presence>([
    ['a', { host_id: 'a', last_seen: '2026-01-01T00:00:00Z', online: true }],
    ['b', { host_id: 'b', last_seen: '2026-01-01T00:00:00Z', online: false }],
  ])

  it('tells online, offline and never heard from apart', () => {
    expect(standingOf(byHost, 'a')).toBe('online')
    expect(standingOf(byHost, 'b')).toBe('offline')
    expect(standingOf(byHost, 'c')).toBe('never')
  })

  it('is unknown, not offline, before presence has loaded', () => {
    expect(standingOf(undefined, 'a')).toBe('unknown')
  })
})

describe('summary', () => {
  it('says how many are online in plain words', () => {
    expect(summary(5, 5)).toBe('All 5 are online')
    expect(summary(4, 5)).toBe('4 of 5 are online')
    expect(summary(0, 5)).toBe('None of the 5 is online')
    expect(summary(1, 1)).toBe('The one host is online')
    expect(summary(0, 1)).toBe('The one host is offline')
    expect(summary(0, 0)).toBe('')
  })
})
