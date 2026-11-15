import { describe, expect, it } from 'vitest'
import { ApiError } from './api'
import { placeError } from './use-action'

describe('placeError', () => {
  const invalid = new ApiError(422, 'invalid', 'is shorter than 10 characters', 'password')

  it('puts a complaint at its field when the form has that field', () => {
    expect(placeError(invalid, ['name', 'password'])).toEqual({ field: 'password', general: undefined })
  })

  it('keeps a complaint about a field the form lacks as a general error', () => {
    expect(placeError(invalid, ['name'])).toEqual({ field: null, general: invalid })
  })

  it('keeps every other failure general', () => {
    const conflict = new ApiError(409, 'conflict', 'the name bob is taken')
    expect(placeError(conflict, ['name'])).toEqual({ field: null, general: conflict })
    expect(placeError(undefined, ['name'])).toEqual({ field: null, general: undefined })
  })
})
