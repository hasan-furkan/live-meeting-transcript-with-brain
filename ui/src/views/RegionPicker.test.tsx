import { fireEvent, render, screen } from '@testing-library/react'
import { expect, test, vi } from 'vitest'

const { close } = vi.hoisted(() => ({ close: vi.fn() }))

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ close }),
}))

vi.mock('../api', () => ({
  saveRegion: vi.fn(async () => {
    throw new Error('disk full')
  }),
}))

import RegionPicker from './RegionPicker'

test('save_region_failure_shows_error_and_keeps_window_open', async () => {
  const { container } = render(<RegionPicker />)
  const overlay = container.querySelector('.region-overlay')
  if (!overlay) throw new Error('region overlay missing')
  fireEvent.mouseDown(overlay, { clientX: 10, clientY: 10 })
  fireEvent.mouseMove(overlay, { clientX: 60, clientY: 80 })
  fireEvent.mouseUp(overlay, { clientX: 60, clientY: 80 })
  fireEvent.click(screen.getByRole('button', { name: 'Save' }))
  expect(await screen.findByText('Error: disk full')).toBeInTheDocument()
  expect(close).not.toHaveBeenCalled()
})
