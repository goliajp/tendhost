import { describe, expect, it } from 'vitest'
import type { Host } from './api'
import { admittedBy, byName, enrollCommand, installCommand, labelToAdd, sendsCredentialInClear, shellQuote } from './rules'

const host = (id: string, name = id, enrollment_id = `enr_${id}`): Host => ({
  id,
  name,
  enrollment_id,
  mode: 'observe',
  labels: [],
  enrolled_at: '2026-01-01T00:00:00Z',
  facts: {
    hostname: name,
    os: 'linux',
    os_version: '6.12',
    arch: 'x86_64',
    cpu_count: 8,
    memory_bytes: 1,
    daemon_version: '0.1.0',
  },
  facts_at: '2026-01-01T00:00:00Z',
})

describe('enrollCommand', () => {
  it('is the daemon command for the server the page was loaded from', () => {
    expect(enrollCommand('https://tend.example.com', 'the_abc-DEF_123', 'observe')).toBe(
      'tendhostd enroll --server https://tend.example.com --token the_abc-DEF_123',
    )
  })

  it('adds --manage only for the mode that allows commands', () => {
    expect(enrollCommand('http://127.0.0.1:7080', 'the_x', 'manage')).toBe(
      'tendhostd enroll --server http://127.0.0.1:7080 --token the_x --manage',
    )
  })
})

describe('shellQuote', () => {
  it('leaves plain values alone and quotes the rest', () => {
    expect(shellQuote('the_abc-123')).toBe('the_abc-123')
    expect(shellQuote('a b')).toBe("'a b'")
    expect(shellQuote("it's")).toBe("'it'\\''s'")
    expect(shellQuote('$(rm -rf /)')).toBe("'$(rm -rf /)'")
  })
})

describe('sendsCredentialInClear', () => {
  it('warns for plain http to another machine only', () => {
    expect(sendsCredentialInClear('http://tend.example.com')).toBe(true)
    expect(sendsCredentialInClear('http://192.168.1.5:7080')).toBe(true)
    expect(sendsCredentialInClear('https://tend.example.com')).toBe(false)
    expect(sendsCredentialInClear('http://localhost:7080')).toBe(false)
    expect(sendsCredentialInClear('http://127.0.0.1:7080')).toBe(false)
  })
})

describe('installCommand', () => {
  it('pipes the script of this server into sh with the token', () => {
    expect(installCommand('https://tend.example.com', 'the_abc', 'observe')).toBe(
      'curl -fsSL https://tend.example.com/install.sh | sh -s -- --token the_abc',
    )
    expect(installCommand('https://tend.example.com', 'the_abc', 'manage')).toBe(
      'curl -fsSL https://tend.example.com/install.sh | sh -s -- --token the_abc --manage',
    )
  })
})

describe('admittedBy', () => {
  it('recognises the host this enrollment admitted, whoever else joined meanwhile', () => {
    const hosts = [host('a', 'a', 'enr_other'), host('b', 'b', 'enr_mine')]
    expect(admittedBy('enr_mine', hosts)?.id).toBe('b')
    expect(admittedBy('enr_unused', hosts)).toBeNull()
  })
})

describe('labelToAdd', () => {
  it('trims, and adds nothing for an empty or repeated label', () => {
    expect(labelToAdd(' web ', ['db'])).toBe('web')
    expect(labelToAdd('   ', [])).toBeNull()
    expect(labelToAdd('db', ['db'])).toBeNull()
  })
})

describe('byName', () => {
  it('orders numbered hosts the way people count', () => {
    const names = [host('1', 'web-10'), host('2', 'web-2'), host('3', 'db')].sort(byName).map((h) => h.name)
    expect(names).toEqual(['db', 'web-2', 'web-10'])
  })
})
