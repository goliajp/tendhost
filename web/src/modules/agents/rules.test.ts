import { describe, expect, it } from 'vitest'
import type { AgentView, UsageDay } from './api'
import { OTHER, byHost, compact, detailArrived, lastDays, offers, seriesModels, stacks, totals, trackedAfter } from './rules'

const view = (host_id: string, agent: AgentView['agent'], tracked = false, accounts = 0): AgentView => ({
  host_id,
  agent,
  account_count: 1,
  supports_usage: true,
  supports_limits: true,
  seen_at: '2026-01-01T00:00:00Z',
  tracked,
  accounts: Array.from({ length: accounts }, (_, i) => ({ dir: `/d${i}`, label: `a${i}`, plan: null })),
  limits: null,
})

const usage = (day: string, model: string, input: number, output = 0, cacheRead = 0, cacheWrite = 0): UsageDay => ({
  day,
  model,
  input_tokens: input,
  output_tokens: output,
  cache_read_tokens: cacheRead,
  cache_write_tokens: cacheWrite,
})

describe('offers', () => {
  it('never promises what the agent does not keep', () => {
    expect(offers({ supports_usage: true, supports_limits: true })).toMatch(/usage per day and limit windows$/)
    expect(offers({ supports_usage: true, supports_limits: false })).toMatch(/keeps no limit readings/)
    expect(offers({ supports_usage: false, supports_limits: true })).toMatch(/keeps no usage/)
    expect(offers({ supports_usage: false, supports_limits: false })).toMatch(/accounts only/)
  })
})

describe('byHost', () => {
  it('groups under hosts by name and drops hosts without agents and agents without a host', () => {
    const groups = byHost(
      [view('h2', 'codex'), view('h1', 'gemini'), view('h1', 'claude'), view('gone', 'kimi')],
      [
        { id: 'h1', name: 'web-10' },
        { id: 'h2', name: 'web-2' },
        { id: 'h3', name: 'idle' },
      ],
    )
    expect(groups.map((g) => [g.host.name, g.agents.map((a) => a.agent)])).toEqual([
      ['web-2', ['codex']],
      ['web-10', ['claude', 'gemini']],
    ])
  })
})

describe('trackedAfter', () => {
  const views = [view('h1', 'claude', true), view('h1', 'codex'), view('h2', 'kimi', true)]
  it('changes one agent and keeps the rest of that host only', () => {
    expect(trackedAfter(views, 'h1', 'codex', true)).toEqual(['claude', 'codex'])
    expect(trackedAfter(views, 'h1', 'claude', false)).toEqual([])
    expect(trackedAfter(views, 'h1', 'claude', true)).toEqual(['claude'])
  })
})

describe('detailArrived', () => {
  it('waits for the accounts of every wanted agent on that host', () => {
    const views = [view('h1', 'claude', true, 1), view('h1', 'codex', true, 0), view('h2', 'codex', true, 2)]
    expect(detailArrived(views, 'h1', ['claude'])).toBe(true)
    expect(detailArrived(views, 'h1', ['claude', 'codex'])).toBe(false)
    expect(detailArrived(views, 'h1', [])).toBe(true)
  })
})

describe('compact', () => {
  it('shortens large counts', () => {
    expect([0, 950, 1200, 34_500, 3_400_000, 250_000_000, 5_600_000_000].map(compact)).toEqual([
      '0',
      '950',
      '1.2k',
      '34.5k',
      '3.4M',
      '250M',
      '5.6B',
    ])
  })
})

describe('usage', () => {
  const days = [
    usage('2026-01-02', 'big', 100, 50, 1000, 10),
    usage('2026-01-02', 'small', 5),
    usage('2026-01-03', 'mid', 40),
    usage('2026-01-03', 'tiny', 1),
  ]

  it('totals each kind of token', () => {
    expect(totals(days)).toEqual({ input: 146, output: 50, cacheRead: 1000, cacheWrite: 10 })
    expect(totals([])).toEqual({ input: 0, output: 0, cacheRead: 0, cacheWrite: 0 })
  })

  it('keeps the busiest models and folds the rest into one series', () => {
    expect(seriesModels(days, 5)).toEqual(['big', 'mid', 'small', 'tiny'])
    expect(seriesModels(days, 3)).toEqual(['big', 'mid', OTHER])
  })

  it('stacks input plus output per day, with zero for a day without sessions', () => {
    const models = seriesModels(days, 3)
    expect(stacks(days, ['2026-01-01', '2026-01-02', '2026-01-03'], models)).toEqual([
      { day: '2026-01-01', parts: [0, 0, 0], total: 0 },
      { day: '2026-01-02', parts: [150, 0, 5], total: 155 },
      { day: '2026-01-03', parts: [0, 40, 1], total: 41 },
    ])
  })

  it('lists the days of a span in utc, oldest first, across a month end', () => {
    expect(lastDays(new Date('2026-03-01T23:30:00Z'), 3)).toEqual(['2026-02-27', '2026-02-28', '2026-03-01'])
  })
})
