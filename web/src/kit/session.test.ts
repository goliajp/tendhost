import { describe, expect, it } from 'vitest'
import { atLeast } from './session'

describe('atLeast', () => {
  it('grants a role itself and everything below', () => {
    expect(atLeast('admin', 'viewer')).toBe(true)
    expect(atLeast('admin', 'admin')).toBe(true)
    expect(atLeast('admin', 'owner')).toBe(false)
    expect(atLeast('viewer', 'admin')).toBe(false)
    expect(atLeast('owner', 'owner')).toBe(true)
  })
})
