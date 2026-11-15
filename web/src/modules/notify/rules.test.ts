import { describe as suite, expect, it } from 'vitest'
import { EMPTY, describe, portAfter, toTarget } from './rules'

suite('toTarget', () => {
  it('builds each kind from its own fields', () => {
    expect(toTarget({ ...EMPTY, kind: 'webhook', url: ' https://example.com/hook ' })).toEqual({
      kind: 'webhook',
      url: 'https://example.com/hook',
    })
    expect(toTarget({ ...EMPTY, kind: 'ntfy', url: 'https://ntfy.sh/topic', token: '' })).toEqual({
      kind: 'ntfy',
      url: 'https://ntfy.sh/topic',
      token: null,
    })
  })

  it('sends an empty optional field as absent and keeps a password as typed', () => {
    const base = { ...EMPTY, kind: 'email' as const, host: ' mail.example.com ', port: '587', security: 'starttls' as const, from: 'a@example.com', to: 'b@example.com' }
    expect(toTarget(base)).toEqual({
      kind: 'email',
      host: 'mail.example.com',
      port: 587,
      security: 'starttls',
      username: null,
      password: null,
      from: 'a@example.com',
      to: 'b@example.com',
    })
    const withLogin = toTarget({ ...base, username: ' me ', password: ' spaced secret ' })
    expect(withLogin.kind === 'email' && [withLogin.username, withLogin.password]).toEqual(['me', ' spaced secret '])
  })
})

suite('describe', () => {
  it('says whether a secret is set and never what it is', () => {
    expect(describe({ kind: 'ntfy', url: 'https://ntfy.sh/t', has_token: true })).toBe('https://ntfy.sh/t, with an access token')
    expect(describe({ kind: 'ntfy', url: 'https://ntfy.sh/t', has_token: false })).toBe('https://ntfy.sh/t, no access token')
    expect(
      describe({ kind: 'email', host: 'mail.example.com', port: 465, security: 'tls', username: 'me', has_password: true, from: 'a@example.com', to: 'b@example.com' }),
    ).toBe('to b@example.com from a@example.com, through mail.example.com:465 (tls from the start), as me')
    expect(
      describe({ kind: 'email', host: 'localhost', port: 25, security: 'none', username: null, has_password: false, from: 'a@example.com', to: 'b@example.com' }),
    ).toMatch(/without signing in$/)
  })
})

suite('portAfter', () => {
  it('follows the security choice until the user sets a port of their own', () => {
    expect(portAfter('tls', 'starttls', '465')).toBe('587')
    expect(portAfter('starttls', 'none', '587')).toBe('25')
    expect(portAfter('tls', 'starttls', '2525')).toBe('2525')
    expect(portAfter('tls', 'none', '')).toBe('25')
  })
})
