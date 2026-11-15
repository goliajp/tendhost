import { describe, expect, it } from 'vitest'
import { grantable, passwordProblem } from './rules'

describe('passwordProblem', () => {
  it('asks for ten characters before it compares the two', () => {
    expect(passwordProblem('123456789', 'x')).toEqual({ password: 'Use at least 10 characters', confirm: null })
    expect(passwordProblem('1234567890', '1234567890')).toEqual({ password: null, confirm: null })
    expect(passwordProblem('1234567890', '123456789x')).toEqual({ password: null, confirm: 'The two passwords differ' })
  })

  it('counts characters the way the server does', () => {
    expect(passwordProblem('密码密码密码密码密码', '密码密码密码密码密码').password).toBeNull()
    expect(passwordProblem('😀😀😀😀😀', '😀😀😀😀😀').password).not.toBeNull()
  })
})

describe('grantable', () => {
  it('never offers a role above the giver', () => {
    expect(grantable('viewer')).toEqual(['viewer'])
    expect(grantable('admin')).toEqual(['viewer', 'admin'])
    expect(grantable('owner')).toEqual(['viewer', 'admin', 'owner'])
  })
})
