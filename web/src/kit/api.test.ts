import { describe, expect, it } from 'vitest'
import { ApiError, explain, splitInvalid, toApiError } from './api'

describe('splitInvalid', () => {
  it('separates the field from the reason', () => {
    expect(splitInvalid('password: is shorter than 10 characters')).toEqual({
      field: 'password',
      reason: 'is shorter than 10 characters',
    })
  })

  it('keeps a message without a field whole', () => {
    expect(splitInvalid('something went wrong')).toEqual({ field: null, reason: 'something went wrong' })
    expect(splitInvalid('two words: then a colon')).toEqual({ field: null, reason: 'two words: then a colon' })
  })
})

describe('toApiError', () => {
  it('reads the server error shape', () => {
    const error = toApiError(409, { error: { code: 'conflict', message: 'the name bob is taken' } })
    expect([error.status, error.code, error.message, error.field]).toEqual([
      409,
      'conflict',
      'the name bob is taken',
      null,
    ])
  })

  it('names the field of an invalid value', () => {
    const error = toApiError(422, {
      error: { code: 'invalid', message: 'setup_token: is not the one the server printed' },
    })
    expect([error.field, error.message]).toEqual(['setup_token', 'is not the one the server printed'])
  })

  it('does not invent a reason when the body is not the error shape', () => {
    expect(toApiError(502, null).message).toBe('The server answered 502 without an explanation')
    expect(toApiError(500, { oops: true }).code).toBe('internal')
  })
})

describe('explain', () => {
  it('says a 403 is about the role', () => {
    expect(explain(new ApiError(403, 'forbidden', 'not allowed'))).toBe('Your role does not allow this')
  })

  it('puts the field back in front of an unplaced invalid message', () => {
    expect(explain(new ApiError(422, 'invalid', 'is empty', 'name'))).toBe('name: is empty')
  })
})
