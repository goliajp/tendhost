import { describe as suite, expect, it } from 'vitest'
import { describe, reachesStart, who } from './describe'

suite('describe', () => {
  it('writes a sentence for the events it knows', () => {
    expect(describe({ topic: 'identity.user_created', payload: { name: 'bob', role: 'viewer' } })).toBe(
      'Added the user bob as viewer',
    )
    expect(describe({ topic: 'host.enrolled', payload: { name: 'web-1', mode: 'manage' } })).toBe(
      'Enrolled as web-1, allowing commands',
    )
    expect(describe({ topic: 'host.updated', payload: { name: 'web-1', labels: ['db', 'web'] } })).toBe(
      'Set the host to web-1; labels db, web',
    )
    expect(describe({ topic: 'host.updated', payload: { name: 'web-1', labels: [] } })).toBe(
      'Set the host to web-1; no labels',
    )
  })

  it('still says what happened when a payload field is missing', () => {
    expect(describe({ topic: 'host.removed', payload: {} })).toBe('Removed the host')
    expect(describe({ topic: 'identity.token_created', payload: { name: 'ci' } })).toBe('Made the API token ci')
  })

  it('shows the topic of an event it does not know', () => {
    expect(describe({ topic: 'jobs.run_finished', payload: { exit: 0 } })).toBe('jobs.run_finished')
  })
})

suite('who', () => {
  it('names the kinds of actor and recognises the reader', () => {
    expect(who({ kind: 'system' }, 'usr_me')).toBe('The server')
    expect(who({ kind: 'host', id: 'host_1' }, 'usr_me')).toBe('Host host_1')
    expect(who({ kind: 'user', id: 'usr_me' }, 'usr_me')).toBe('You')
    expect(who({ kind: 'user', id: 'usr_other' }, 'usr_me')).toBe('User usr_other')
  })
})

suite('reachesStart', () => {
  it('stops offering older entries at the first one or after a short page', () => {
    expect(reachesStart([{ seq: 3 }, { seq: 2 }], 2, 2)).toBe(false)
    expect(reachesStart([{ seq: 2 }, { seq: 1 }], 2, 2)).toBe(true)
    expect(reachesStart([{ seq: 9 }], 1, 2)).toBe(true)
    expect(reachesStart([], 0, 2)).toBe(true)
  })
})
