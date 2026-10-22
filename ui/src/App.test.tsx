import { render, screen } from '@testing-library/react'
import { expect, test, vi } from 'vitest'

vi.mock('./api', () => ({
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
  hasApiKey: vi.fn(async () => false),
  setApiKey: vi.fn(async () => {}),
  listCourses: vi.fn(async () => []),
  startSession: vi.fn(async () => {}),
  stopSession: vi.fn(async () => {}),
  askNow: vi.fn(async () => {}),
  retrySummary: vi.fn(async () => {}),
  modelStatus: vi.fn(async () => ({ installed: false, model: 'small' })),
  downloadModel: vi.fn(async () => {}),
  listMonitors: vi.fn(async () => []),
  listOutputDevices: vi.fn(async () => []),
  openRegionPicker: vi.fn(async () => {}),
  saveRegion: vi.fn(async () => {}),
  onSessionEvent: vi.fn(() => Promise.resolve(() => {})),
  onModelProgress: vi.fn(() => Promise.resolve(() => {})),
}))

import App from './App'

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
