import { describe, expect, it } from 'vitest'
import { fromForm, ratioOf, toForm } from './rules'

describe('ratioOf', () => {
  it('reads a whole percentage from 1 to 100', () => {
    expect(ratioOf('90')).toBe(0.9)
    expect(ratioOf(' 1 ')).toBe(0.01)
    expect(ratioOf('100')).toBe(1)
  })

  it('refuses everything else', () => {
    for (const bad of ['0', '101', '', '9.5', '90%', 'ninety', '-5']) expect(ratioOf(bad)).toBeNull()
  })
})

describe('the settings form', () => {
  const settings = { host_offline: true, disk_usage: 0.85, agent_limit: null, job_failed: false }

  it('shows thresholds in percent, and a default level for one that is off', () => {
    expect(toForm(settings)).toEqual({
      hostOffline: true,
      jobFailed: false,
      diskOn: true,
      diskPercent: '85',
      agentOn: false,
      agentPercent: '90',
    })
  })

  it('goes back to the same settings', () => {
    expect(fromForm(toForm(settings))).toEqual({ ok: true, settings })
  })

  it('sends null for a condition switched off, whatever is left in its box', () => {
    const parsed = fromForm({ ...toForm(settings), diskOn: false, diskPercent: 'junk' })
    expect(parsed).toEqual({ ok: true, settings: { ...settings, disk_usage: null } })
  })

  it('names the threshold that is not a percentage', () => {
    expect(fromForm({ ...toForm(settings), diskPercent: '0' })).toEqual({ ok: false, field: 'disk_usage' })
    expect(fromForm({ ...toForm(settings), agentOn: true, agentPercent: '150' })).toEqual({
      ok: false,
      field: 'agent_limit',
    })
  })
})
