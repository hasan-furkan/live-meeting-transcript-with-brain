import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, test, vi } from 'vitest'

const { close, show, regionSnapshot, saveRegion } = vi.hoisted(() => ({
  close: vi.fn(),
  show: vi.fn(),
  regionSnapshot: vi.fn(),
  saveRegion: vi.fn(),
}))

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ close, show }),
}))

vi.mock('../api', () => ({ regionSnapshot, saveRegion }))

import RegionPicker from './RegionPicker'
import type { RegionSnapshot } from '../bindings/RegionSnapshot'

const snapshot: RegionSnapshot = {
  image_base64: 'AAAA',
  media_type: 'image/jpeg',
  monitor_width: 3840,
  monitor_height: 2160,
}

afterEach(() => {
  Reflect.deleteProperty(HTMLImageElement.prototype, 'clientWidth')
  Reflect.deleteProperty(HTMLImageElement.prototype, 'clientHeight')
  close.mockReset()
  regionSnapshot.mockReset()
  saveRegion.mockReset()
})

test('shows_loading_and_cancel_before_snapshot', () => {
  regionSnapshot.mockReturnValue(new Promise(() => {}))
  render(<RegionPicker />)
  expect(screen.getByText('Taking a screenshot…')).toBeInTheDocument()
  expect(screen.getByRole('button', { name: 'Cancel' })).toBeInTheDocument()
  expect(screen.queryByRole('img')).not.toBeInTheDocument()
  expect(saveRegion).not.toHaveBeenCalled()
})

test('snapshot_error_shows_close_button', async () => {
  regionSnapshot.mockRejectedValue(new Error('no screen'))
  render(<RegionPicker />)
  expect(await screen.findByText('Error: no screen')).toBeInTheDocument()
  fireEvent.click(screen.getByRole('button', { name: 'Close' }))
  expect(close).toHaveBeenCalled()
})

test('drag_on_image_saves_physical_pixels', async () => {
  regionSnapshot.mockResolvedValue(snapshot)
  saveRegion.mockResolvedValue(undefined)
  render(<RegionPicker />)
  const img = await screen.findByRole('img')
  Object.defineProperty(HTMLImageElement.prototype, 'clientWidth', { value: 1920 })
  Object.defineProperty(HTMLImageElement.prototype, 'clientHeight', { value: 1080 })

  fireEvent.mouseDown(img, { clientX: 100, clientY: 100 })
  fireEvent.mouseMove(img, { clientX: 300, clientY: 200 })
  fireEvent.mouseUp(img, { clientX: 300, clientY: 200 })
  fireEvent.click(screen.getByRole('button', { name: 'Save' }))

  await waitFor(() =>
    expect(saveRegion).toHaveBeenCalledWith({ x: 200, y: 200, width: 400, height: 200 }),
  )
})

test('save_region_failure_shows_error_and_keeps_window_open', async () => {
  regionSnapshot.mockResolvedValue(snapshot)
  saveRegion.mockRejectedValue(new Error('disk full'))
  render(<RegionPicker />)
  const img = await screen.findByRole('img')
  Object.defineProperty(HTMLImageElement.prototype, 'clientWidth', { value: 1920 })
  Object.defineProperty(HTMLImageElement.prototype, 'clientHeight', { value: 1080 })

  fireEvent.mouseDown(img, { clientX: 10, clientY: 10 })
  fireEvent.mouseMove(img, { clientX: 60, clientY: 80 })
  fireEvent.mouseUp(img, { clientX: 60, clientY: 80 })
  fireEvent.click(screen.getByRole('button', { name: 'Save' }))
  expect(await screen.findByText('Error: disk full')).toBeInTheDocument()
  expect(close).not.toHaveBeenCalled()
})
