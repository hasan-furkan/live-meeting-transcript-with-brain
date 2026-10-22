import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, expect, test, vi } from 'vitest'

vi.mock('../api', () => ({
  getSettings: vi.fn(async () => ({
    api_base_url: 'https://example.test',
    model: 'test-model',
    whisper_model: 'small',
    language: 'en',
    notes_dir: null,
    name_variants: [],
    keywords: [],
    general_questions: true,
    name_bias: true,
    hotkey: 'CommandOrControl+Shift+Space',
    screenshot_mode: 'off',
    monitor: null,
    region: null,
    audio_device: null,
  })),
  saveSettings: vi.fn(async () => {}),
  hasApiKey: vi.fn(async () => true),
  setApiKey: vi.fn(async () => {}),
  modelStatus: vi.fn(async () => ({ installed: true, model: 'small' })),
  downloadModel: vi.fn(async () => {}),
  listMonitors: vi.fn(async () => ['Display 1']),
  listOutputDevices: vi.fn(async () => ['Speakers']),
  openRegionPicker: vi.fn(async () => {}),
  onModelProgress: vi.fn(() => Promise.resolve(() => {})),
}))

import * as api from '../api'
import Settings from './Settings'

beforeEach(() => {
  vi.clearAllMocks()
})

test('api_key_input_not_prefilled', async () => {
  render(<Settings />)
  await screen.findByText('Key saved')
  const input = screen.getByLabelText('API key')
  expect(input).toHaveValue('')
  expect(input).toHaveAttribute('type', 'password')
})

test('refocus_keeps_region_from_picker_and_unsaved_edits', async () => {
  const region = { x: 10, y: 20, width: 300, height: 200 }
  render(<Settings />)
  await screen.findByText('No region selected')
  fireEvent.change(screen.getByLabelText('Model'), { target: { value: 'edited-model' } })
  vi.mocked(api.getSettings).mockResolvedValueOnce({
    api_base_url: 'https://example.test',
    model: 'test-model',
    whisper_model: 'small',
    language: 'en',
    notes_dir: null,
    name_variants: [],
    keywords: [],
    general_questions: true,
    name_bias: true,
    hotkey: 'CommandOrControl+Shift+Space',
    screenshot_mode: 'off',
    monitor: null,
    region,
    audio_device: null,
  })
  fireEvent(window, new Event('focus'))
  await screen.findByText('Region: 10, 20 300×200')
  fireEvent.click(screen.getByRole('button', { name: 'Save' }))
  await waitFor(() => expect(api.saveSettings).toHaveBeenCalled())
  const saved = vi.mocked(api.saveSettings).mock.calls[0]?.[0]
  expect(saved?.region).toEqual(region)
  expect(saved?.model).toBe('edited-model')
})

test('settings_save_sends_trimmed_nonempty_variants', async () => {
  render(<Settings />)
  fireEvent.change(await screen.findByLabelText('Name variants'), {
    target: { value: ' Jordan Reyes \n\n Al ' },
  })
  fireEvent.change(screen.getByLabelText('Keywords'), { target: { value: 'exam\n\n' } })
  fireEvent.click(screen.getByRole('button', { name: 'Save' }))
  await waitFor(() => expect(api.saveSettings).toHaveBeenCalled())
  const saved = vi.mocked(api.saveSettings).mock.calls[0]?.[0]
  expect(saved?.name_variants).toEqual(['Jordan Reyes', 'Al'])
  expect(saved?.keywords).toEqual(['exam'])
  expect(api.setApiKey).not.toHaveBeenCalled()
})
