import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, expect, test, vi } from 'vitest'

vi.mock('../api', () => ({
  getSettings: vi.fn(async () => ({
    api_base_url: 'https://example.test',
    model: 'test-model',
    whisper_model: 'small',
    language: 'en',
    explanation_language: '',
    notes_dir: null,
    name_variants: [],
    keywords: [],
    general_questions: true,
    name_bias: true,
    hotkey: 'CommandOrControl+Shift+Space',
    screenshot_mode: 'off',
    monitor: null,
    region: null,
    window: null,
    audio_device: null,
  })),
  saveSettings: vi.fn(async () => {}),
  hasApiKey: vi.fn(async () => true),
  setApiKey: vi.fn(async () => {}),
  modelStatus: vi.fn(async () => ({ installed: true, model: 'small' })),
  downloadModel: vi.fn(async () => {}),
  listMonitors: vi.fn(async () => ['Display 1']),
  listWindows: vi.fn(async () => ({ windows: [], current: null })),
  backgroundCaptureHint: vi.fn(async () => ({
    text: 'Chromium browsers may stop drawing a window you are not looking at. Not tested with Microsoft Teams.',
    command: 'chrome.exe --disable-backgrounding-occluded-windows',
  })),
  listOutputDevices: vi.fn(async () => ['Speakers']),
  openRegionPicker: vi.fn(async () => {}),
  onModelProgress: vi.fn(() => Promise.resolve(() => {})),
}))

import * as api from '../api'
import type { Settings as SettingsData } from '../bindings/Settings'
import type { WindowList } from '../bindings/WindowList'
import Settings from './Settings'

const baseSettings: SettingsData = {
  api_base_url: 'https://example.test',
  model: 'test-model',
  whisper_model: 'small',
  language: 'en',
  explanation_language: '',
  notes_dir: null,
  name_variants: [],
  keywords: [],
  general_questions: true,
  name_bias: true,
  hotkey: 'CommandOrControl+Shift+Space',
  screenshot_mode: 'off',
  monitor: null,
  region: null,
  window: null,
  audio_device: null,
}

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
    explanation_language: '',
    notes_dir: null,
    name_variants: [],
    keywords: [],
    general_questions: true,
    name_bias: true,
    hotkey: 'CommandOrControl+Shift+Space',
    screenshot_mode: 'off',
    monitor: null,
    region,
    window: null,
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

test('editing_shows_unsaved_and_saving_shows_saved', async () => {
  render(<Settings />)
  await screen.findByText('Key saved')
  expect(screen.queryByText('Unsaved changes — press Save (or Enter)')).toBeNull()
  fireEvent.change(screen.getByLabelText('Model'), { target: { value: 'edited-model' } })
  expect(screen.getByText('Unsaved changes — press Save (or Enter)')).toBeInTheDocument()
  fireEvent.click(screen.getByRole('button', { name: 'Save' }))
  expect(await screen.findByText('Saved ✓')).toBeInTheDocument()
  expect(screen.queryByText('Unsaved changes — press Save (or Enter)')).toBeNull()
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

test('window_mode_lists_windows_and_saves_the_choice', async () => {
  vi.mocked(api.listWindows).mockResolvedValue({
    windows: [
      { id: 1, app: 'Zoom', title: 'Algorithms 101' },
      { id: 2, app: 'Firefox', title: 'Course notes' },
    ],
    current: null,
  })
  render(<Settings />)
  await screen.findByText('Key saved')
  fireEvent.click(screen.getByRole('button', { name: 'Window' }))

  const select = await screen.findByRole('combobox', { name: 'Window' })
  expect(select).toHaveAttribute('id', 'window-select')
  fireEvent.click(select)
  expect(
    await screen.findByRole('option', { name: 'Zoom — Algorithms 101' }),
  ).toBeInTheDocument()
  expect(screen.getByRole('option', { name: 'Firefox — Course notes' })).toBeInTheDocument()
  expect(screen.queryByRole('button', { name: 'Select region' })).toBeNull()

  fireEvent.click(screen.getByRole('option', { name: 'Firefox — Course notes' }))
  fireEvent.click(screen.getByRole('button', { name: 'Save' }))
  await waitFor(() => expect(api.saveSettings).toHaveBeenCalled())
  const saved = vi.mocked(api.saveSettings).mock.calls[0]?.[0]
  expect(saved?.window).toEqual({ id: 2, app: 'Firefox', title: 'Course notes' })
})

test('window_mode_shows_list_windows_error', async () => {
  vi.mocked(api.listWindows).mockRejectedValue('window list unavailable')
  render(<Settings />)
  await screen.findByText('Key saved')
  fireEvent.click(screen.getByRole('button', { name: 'Window' }))
  expect(await screen.findByText('window list unavailable')).toBeInTheDocument()
})

test('window_mode_hides_a_closed_saved_window_and_says_so', async () => {
  vi.mocked(api.getSettings).mockResolvedValueOnce({
    ...baseSettings,
    screenshot_mode: 'window',
    window: { id: 9, app: 'Zoom', title: 'Gone' },
  })
  vi.mocked(api.listWindows).mockResolvedValue({
    windows: [{ id: 1, app: 'Zoom', title: 'Algorithms 101' }],
    current: null,
  })
  render(<Settings />)

  const select = await screen.findByRole('combobox', { name: 'Window' })
  expect(select).toHaveTextContent('Choose a window…')
  fireEvent.click(select)
  await screen.findByRole('option', { name: 'Zoom — Algorithms 101' })
  expect(screen.queryByRole('option', { name: /Gone/ })).toBeNull()
  expect(screen.queryByText(/not open now/)).toBeNull()
  expect(
    await screen.findByText('The saved window is not open. Open it, or choose another.'),
  ).toBeInTheDocument()
})

test('window_option_names_the_whole_window_but_shows_the_tab_as_detail', async () => {
  vi.mocked(api.getSettings).mockResolvedValueOnce({
    ...baseSettings,
    screenshot_mode: 'window',
  })
  vi.mocked(api.listWindows).mockResolvedValue({
    windows: [{ id: 3, app: 'Google Chrome', title: 'WhatsApp - Google Chrome' }],
    current: null,
  })
  render(<Settings />)

  const select = await screen.findByRole('combobox', { name: 'Window' })
  fireEvent.click(select)
  const option = await screen.findByRole('option', {
    name: 'Google Chrome — WhatsApp - Google Chrome',
  })
  expect(option).toHaveTextContent('WhatsApp')
  expect(option).not.toHaveTextContent(' - Google Chrome')
})

test('show_windows_on_other_desktops_refreshes_with_hidden_windows', async () => {
  vi.mocked(api.getSettings).mockResolvedValueOnce({
    ...baseSettings,
    screenshot_mode: 'window',
  })
  vi.mocked(api.listWindows).mockResolvedValue({ windows: [], current: null })
  render(<Settings />)

  fireEvent.click(
    await screen.findByLabelText('Also show minimized windows and other desktops'),
  )
  await waitFor(() => expect(api.listWindows).toHaveBeenCalledWith(true, null))
})

test('window_hint_appears_only_in_window_mode', async () => {
  render(<Settings />)
  await screen.findByText('Key saved')
  await waitFor(() => expect(api.backgroundCaptureHint).toHaveBeenCalled())
  expect(screen.queryByText('Window comes back black?')).toBeNull()

  fireEvent.click(screen.getByRole('button', { name: 'Window' }))
  expect(await screen.findByText('Window comes back black?')).toBeInTheDocument()
  expect(
    screen.getByText('chrome.exe --disable-backgrounding-occluded-windows'),
  ).toBeInTheDocument()
})

test('copy_button_writes_the_command_and_reads_copied', async () => {
  const writeText = vi.fn(async () => {})
  Object.defineProperty(navigator, 'clipboard', {
    value: { writeText },
    configurable: true,
  })
  render(<Settings />)
  fireEvent.click(await screen.findByRole('button', { name: 'Window' }))

  fireEvent.click(await screen.findByRole('button', { name: 'Copy' }))
  expect(await screen.findByRole('button', { name: 'Copied' })).toBeInTheDocument()
  expect(writeText).toHaveBeenCalledWith('chrome.exe --disable-backgrounding-occluded-windows')
})

test('a_failed_hint_fetch_hides_the_section', async () => {
  vi.mocked(api.backgroundCaptureHint).mockRejectedValueOnce('unavailable')
  render(<Settings />)
  fireEvent.click(await screen.findByRole('button', { name: 'Window' }))
  await waitFor(() => expect(api.backgroundCaptureHint).toHaveBeenCalled())
  expect(screen.queryByText('Window comes back black?')).toBeNull()
})

test('window_mode_refreshes_the_list_when_the_app_window_gets_focus', async () => {
  vi.mocked(api.getSettings).mockResolvedValueOnce({
    ...baseSettings,
    screenshot_mode: 'window',
  })
  vi.mocked(api.listWindows).mockResolvedValue({ windows: [], current: null })
  render(<Settings />)

  await waitFor(() => expect(api.listWindows).toHaveBeenCalled())
  const calls = vi.mocked(api.listWindows).mock.calls.length
  fireEvent(window, new Event('focus'))
  await waitFor(() => expect(vi.mocked(api.listWindows).mock.calls.length).toBeGreaterThan(calls))
})

test('a_slow_older_window_list_reply_does_not_overwrite_a_newer_one', async () => {
  vi.mocked(api.getSettings).mockResolvedValueOnce({
    ...baseSettings,
    screenshot_mode: 'window',
  })
  const newer: WindowList = {
    windows: [{ id: 2, app: 'Zoom', title: 'New reply' }],
    current: null,
  }
  const older: WindowList = {
    windows: [{ id: 1, app: 'Zoom', title: 'Old reply' }],
    current: null,
  }
  // The refresh from the first load stays in flight; every later one answers at once.
  let resolveOlder: (list: WindowList) => void = () => {}
  let requests = 0
  vi.mocked(api.listWindows).mockImplementation(() => {
    requests += 1
    if (requests === 1) {
      return new Promise<WindowList>((resolve) => {
        resolveOlder = resolve
      })
    }
    return Promise.resolve(newer)
  })

  render(<Settings />)
  await waitFor(() => expect(requests).toBe(1))
  fireEvent.click(
    await screen.findByLabelText('Also show minimized windows and other desktops'),
  )
  fireEvent.click(await screen.findByRole('combobox', { name: 'Window' }))
  expect(await screen.findByRole('option', { name: 'Zoom — New reply' })).toBeInTheDocument()

  await act(async () => {
    resolveOlder(older)
  })
  expect(screen.queryByRole('option', { name: 'Zoom — Old reply' })).toBeNull()
  expect(screen.getByRole('option', { name: 'Zoom — New reply' })).toBeInTheDocument()
})
