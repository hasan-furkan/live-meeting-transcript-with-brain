import { act, fireEvent, render, screen } from '@testing-library/react'
import { expect, test, vi } from 'vitest'
import type { SessionEvent } from './bindings/SessionEvent'

vi.mock('./api', () => ({
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
    audio_device: null,
  })),
  saveSettings: vi.fn(async () => {}),
  hasApiKey: vi.fn(async () => false),
  setApiKey: vi.fn(async () => {}),
  listCourses: vi.fn(async () => []),
  startSession: vi.fn(async () => {}),
  stopSession: vi.fn(async () => {}),
  askNow: vi.fn(async () => {}),
  retrySummary: vi.fn(async () => {}),
  openScreenRecordingSettings: vi.fn(async () => {}),
  modelStatus: vi.fn(async () => ({ installed: false, model: 'small' })),
  downloadModel: vi.fn(async () => {}),
  listMonitors: vi.fn(async () => []),
  listWindows: vi.fn(async () => []),
  listOutputDevices: vi.fn(async () => []),
  openRegionPicker: vi.fn(async () => {}),
  saveRegion: vi.fn(async () => {}),
  onSessionEvent: vi.fn(() => Promise.resolve(() => {})),
  onModelProgress: vi.fn(() => Promise.resolve(() => {})),
}))

import * as api from './api'
import App from './App'

function send(event: SessionEvent) {
  const handler = vi.mocked(api.onSessionEvent).mock.calls[0]?.[0]
  if (!handler) throw new Error('Main did not subscribe to session events')
  act(() => handler(event))
}

test('renders the main view by default', () => {
  window.location.hash = ''
  render(<App />)
  expect(screen.getByRole('heading', { name: 'Live Meeting Assistant' })).toBeInTheDocument()
})

test('settings hash renders the settings view', async () => {
  window.location.hash = '#/settings'
  render(<App />)
  expect(await screen.findByRole('heading', { name: 'Settings' })).toBeInTheDocument()
  window.location.hash = ''
})

test('session_state_survives_settings_navigation', async () => {
  vi.mocked(api.listCourses).mockResolvedValue(['Algorithms 101'])
  window.location.hash = ''
  render(<App />)

  fireEvent.click(screen.getByRole('button', { name: 'Start' }))
  fireEvent.click(await screen.findByRole('button', { name: 'Algorithms 101' }))
  await screen.findByRole('button', { name: 'Stop' })

  window.location.hash = '#/settings'
  act(() => window.dispatchEvent(new HashChangeEvent('hashchange')))
  await screen.findByRole('heading', { name: 'Settings' })

  send({ type: 'Line', time: '10:00', text: 'a line from the lecture' })

  window.location.hash = ''
  act(() => window.dispatchEvent(new HashChangeEvent('hashchange')))
  expect(await screen.findByRole('button', { name: 'Stop' })).toBeInTheDocument()
  expect(screen.getByText('a line from the lecture')).toBeInTheDocument()
})
