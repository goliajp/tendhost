import { cleanup, render } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { Slot, SlotProvider, type ModuleDef } from './module'

afterEach(cleanup)

const page: ModuleDef = { id: 'page', routes: [], nav: [] }
const first: ModuleDef = {
  id: 'first',
  routes: [],
  nav: [],
  fills: { 'host.state': ({ hostId }) => <span>first sees {hostId}</span> },
}
const second: ModuleDef = {
  id: 'second',
  routes: [],
  nav: [],
  fills: {
    'host.state': ({ hostId }) => <span>second sees {hostId}</span>,
    'hosts.summary': ({ hostIds }) => <span>{hostIds.length} hosts</span>,
  },
}

describe('Slot', () => {
  it('renders what every module put into it, in registry order, with the page props', () => {
    const { container } = render(
      <SlotProvider modules={[page, first, second]}>
        <Slot name="host.state" hostId="host_1" />
      </SlotProvider>,
    )
    expect(container.textContent).toBe('first sees host_1second sees host_1')
  })

  it('keeps slots apart', () => {
    const { container } = render(
      <SlotProvider modules={[first, second]}>
        <Slot name="hosts.summary" hostIds={['a', 'b']} />
      </SlotProvider>,
    )
    expect(container.textContent).toBe('2 hosts')
  })

  it('renders nothing when no module fills the slot, so a page works without the others', () => {
    const { container } = render(
      <SlotProvider modules={[page]}>
        <Slot name="host.sections" hostId="host_1" />
      </SlotProvider>,
    )
    expect(container.innerHTML).toBe('')
  })
})
