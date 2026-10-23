import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, expect, test, vi } from 'vitest'
import type { SessionEvent } from '../bindings/SessionEvent'

vi.mock('../api', () => ({
  onSessionEvent: vi.fn(() => Promise.resolve(() => {})),
  listCourses: vi.fn(async () => []),
  startSession: vi.fn(async () => {}),
  stopSession: vi.fn(async () => {}),
  askNow: vi.fn(async () => {}),
  captureAndAsk: vi.fn(async () => {}),
  retrySummary: vi.fn(async () => {}),
  openScreenRecordingSettings: vi.fn(async () => {}),
}))

import * as api from '../api'
import Main from './Main'

function send(event: SessionEvent) {
  const handler = vi.mocked(api.onSessionEvent).mock.calls[0]?.[0]
  if (!handler) throw new Error('Main did not subscribe to session events')
  act(() => handler(event))
}

beforeEach(() => {
  vi.clearAllMocks()
})

test('start_opens_course_picker_and_calls_start_session', async () => {
  vi.mocked(api.listCourses).mockResolvedValue(['Algorithms 101'])
  render(<Main />)
  fireEvent.click(screen.getByRole('button', { name: 'Start' }))
  fireEvent.click(await screen.findByRole('button', { name: 'Algorithms 101' }))
  await waitFor(() => expect(api.startSession).toHaveBeenCalledWith('Algorithms 101'))
  await screen.findByRole('button', { name: 'Stop' })
})

test('analysis_event_renders_card_newest_first', () => {
  render(<Main />)
  send({ type: 'AnalysisStarted', id: 1, reason: 'name' })
  send({
    type: 'Analysis',
    id: 1,
    time: '10:00',
    analysis: {
      question: 'Q1',
      suggested_answer: 'A1',
      confidence: 'high',
      explanation: null,
      raw: null,
    },
    screenshot: 'off',
    image: null,
  })
  send({ type: 'AnalysisStarted', id: 2, reason: 'question' })
  send({
    type: 'Analysis',
    id: 2,
    time: '10:01',
    analysis: {
      question: 'Q2',
      suggested_answer: 'A2',
      confidence: 'medium',
      explanation: null,
      raw: null,
    },
    screenshot: 'off',
    image: null,
  })
  send({ type: 'AnalysisStarted', id: 3, reason: 'question' })
  send({
    type: 'Analysis',
    id: 3,
    time: '10:02',
    analysis: {
      question: '',
      suggested_answer: 'A3',
      confidence: null,
      explanation: null,
      raw: 'raw text',
    },
    screenshot: 'off',
    image: null,
  })
  const cards = screen.getAllByTestId('analysis-card')
  expect(cards[0]).toHaveTextContent('raw text')
  expect(cards[1]).toHaveTextContent('Q2')
  expect(cards[2]).toHaveTextContent('Q1')
  expect(cards[2]).toHaveTextContent('high')
})

test('analysis_card_shows_the_screenshot_status', () => {
  render(<Main />)
  const analysis = {
    question: 'Q',
    suggested_answer: 'A',
    confidence: null,
    explanation: null,
    raw: null,
  }
  send({ type: 'AnalysisStarted', id: 1, reason: 'manual' })
  send({ type: 'Analysis', id: 1, time: '10:00', analysis, screenshot: 'attached', image: null })
  send({ type: 'AnalysisStarted', id: 2, reason: 'manual' })
  send({ type: 'Analysis', id: 2, time: '10:01', analysis, screenshot: 'failed', image: null })
  send({ type: 'AnalysisStarted', id: 3, reason: 'manual' })
  send({ type: 'Analysis', id: 3, time: '10:02', analysis, screenshot: 'off', image: null })
  const cards = screen.getAllByTestId('analysis-card')
  expect(cards[0]).not.toHaveTextContent('screenshot')
  expect(cards[1]).toHaveTextContent('📷 screenshot failed')
  expect(cards[2]).toHaveTextContent('📷 with screenshot')
})

test('capture_and_ask_button_calls_the_command_while_running', async () => {
  vi.mocked(api.listCourses).mockResolvedValue(['Algorithms 101'])
  render(<Main />)
  fireEvent.click(screen.getByRole('button', { name: 'Start' }))
  fireEvent.click(await screen.findByRole('button', { name: 'Algorithms 101' }))
  await screen.findByRole('button', { name: 'Stop' })
  const button = screen.getByRole('button', { name: 'Capture & ask' })
  expect(button).toHaveAttribute(
    'title',
    'Takes a screenshot of the selected region (or the whole screen) and asks about it',
  )
  fireEvent.click(button)
  await waitFor(() => expect(api.captureAndAsk).toHaveBeenCalled())
})

test('screenshot_thumbnail_opens_and_closes_the_overlay', () => {
  render(<Main />)
  const url = 'data:image/jpeg;base64,AAAA'
  send({ type: 'AnalysisStarted', id: 1, reason: 'capture' })
  send({
    type: 'Analysis',
    id: 1,
    time: '10:00',
    analysis: {
      question: 'Q',
      suggested_answer: 'A',
      confidence: null,
      explanation: null,
      raw: null,
    },
    screenshot: 'attached',
    image: url,
  })
  const alt = 'Screenshot sent with the question'
  expect(screen.getAllByAltText(alt)).toHaveLength(1)
  fireEvent.click(screen.getByAltText(alt))
  const overlayImages = screen.getAllByAltText(alt)
  expect(overlayImages).toHaveLength(2)
  fireEvent.keyDown(window, { key: 'Escape' })
  expect(screen.getAllByAltText(alt)).toHaveLength(1)
})

test('explanation_renders_only_when_present', () => {
  render(<Main />)
  send({ type: 'AnalysisStarted', id: 1, reason: 'manual' })
  send({
    type: 'Analysis',
    id: 1,
    time: '10:00',
    analysis: {
      question: 'Q1',
      suggested_answer: 'A1',
      confidence: null,
      explanation: 'The question asks what a heap is.',
      raw: null,
    },
    screenshot: 'off',
    image: null,
  })
  expect(screen.getByText('The question asks what a heap is.')).toBeInTheDocument()

  send({ type: 'AnalysisStarted', id: 2, reason: 'manual' })
  send({
    type: 'Analysis',
    id: 2,
    time: '10:01',
    analysis: {
      question: 'Q2',
      suggested_answer: 'A2',
      confidence: null,
      explanation: null,
      raw: null,
    },
    screenshot: 'off',
    image: null,
  })
  const cards = screen.getAllByTestId('analysis-card')
  expect(cards[0]).toHaveTextContent('A2')
  expect(cards[0]).not.toHaveTextContent('The question asks what a heap is.')
})

test('transcript_text_with_html_is_escaped', () => {
  render(<Main />)
  send({ type: 'Line', time: '10:00', text: '<img src=x onerror=alert(1)>' })
  expect(screen.getByText('<img src=x onerror=alert(1)>')).toBeInTheDocument()
  expect(document.querySelector('img')).toBeNull()
})

test('cpu_backend_shows_warning', () => {
  render(<Main />)
  send({ type: 'Backend', backend: 'Cpu' })
  expect(screen.getByText('CPU mode — slow')).toBeInTheDocument()
})

test('no_audio_banner_clears_on_level', () => {
  render(<Main />)
  send({ type: 'NoAudio' })
  expect(screen.getByText('No audio detected — check the output device')).toBeInTheDocument()
  send({ type: 'Level', rms: 0.05 })
  expect(screen.queryByText('No audio detected — check the output device')).toBeNull()
})

test('unreadable_reply_shows_raw_text_without_suggested_answer', () => {
  render(<Main />)
  send({ type: 'AnalysisStarted', id: 1, reason: 'question' })
  send({
    type: 'Analysis',
    id: 1,
    time: '10:00',
    analysis: {
      question: '',
      suggested_answer: '',
      confidence: null,
      explanation: null,
      raw: 'I could not answer that.',
    },
    screenshot: 'off',
    image: null,
  })
  expect(screen.getByText('Unreadable reply from the model')).toBeInTheDocument()
  expect(screen.getByText('I could not answer that.')).toBeInTheDocument()
  expect(screen.queryByText('Suggested answer')).toBeNull()
})

test('parsed_reply_shows_question_and_suggested_answer', () => {
  render(<Main />)
  send({ type: 'AnalysisStarted', id: 1, reason: 'question' })
  send({
    type: 'Analysis',
    id: 1,
    time: '10:00',
    analysis: {
      question: 'What is a heap?',
      suggested_answer: 'A tree with the heap property.',
      confidence: 'high',
      explanation: null,
      raw: null,
    },
    screenshot: 'off',
    image: null,
  })
  expect(screen.getByText('What is a heap?')).toBeInTheDocument()
  expect(screen.getByText('A tree with the heap property.')).toBeInTheDocument()
  expect(screen.getByText('Suggested answer')).toBeInTheDocument()
  expect(screen.queryByText('Unreadable reply from the model')).toBeNull()
})

test('raw_reply_html_is_escaped', () => {
  render(<Main />)
  send({ type: 'AnalysisStarted', id: 1, reason: 'question' })
  send({
    type: 'Analysis',
    id: 1,
    time: '10:00',
    analysis: {
      question: '',
      suggested_answer: '',
      confidence: null,
      explanation: null,
      raw: '<img src=x onerror=alert(1)>',
    },
    screenshot: 'off',
    image: null,
  })
  expect(screen.getByText('<img src=x onerror=alert(1)>')).toBeInTheDocument()
  expect(document.querySelector('img')).toBeNull()
})

test('no_audio_banner_clears_on_line_without_level', () => {
  render(<Main />)
  send({ type: 'NoAudio' })
  expect(screen.getByText('No audio detected — check the output device')).toBeInTheDocument()
  send({ type: 'Line', time: '10:00', text: 'Hello class' })
  expect(screen.queryByText('No audio detected — check the output device')).toBeNull()
})

test('no_audio_banner_clears_on_partial', () => {
  render(<Main />)
  send({ type: 'NoAudio' })
  expect(screen.getByText('No audio detected — check the output device')).toBeInTheDocument()
  send({ type: 'Partial', text: 'Hello' })
  expect(screen.queryByText('No audio detected — check the output device')).toBeNull()
})

test('audio_level_meter_maps_rms_to_db_and_percent', () => {
  render(<Main />)
  const meter = screen.getByRole('meter', { name: 'Audio level' })
  expect(meter).toHaveAttribute('aria-valuetext', '—')
  expect(meter).toHaveAttribute('aria-valuenow', '0')

  send({ type: 'Level', rms: 0.1 })
  expect(meter).toHaveAttribute('aria-valuetext', '-20 dB')
  expect(meter).toHaveAttribute('aria-valuenow', '67')

  send({ type: 'Level', rms: 0 })
  expect(meter).toHaveAttribute('aria-valuetext', '—')
  expect(meter).toHaveAttribute('aria-valuenow', '0')

  send({ type: 'Level', rms: 2 })
  expect(meter).toHaveAttribute('aria-valuenow', '100')
})

test('permission_denied_explains_and_opens_system_settings', async () => {
  vi.mocked(api.listCourses).mockResolvedValue(['Algorithms 101'])
  vi.mocked(api.startSession).mockRejectedValueOnce('Screen Recording permission required')
  render(<Main />)
  fireEvent.click(screen.getByRole('button', { name: 'Start' }))
  fireEvent.click(await screen.findByRole('button', { name: 'Algorithms 101' }))
  expect(
    await screen.findByText(
      'macOS needs Screen Recording permission to capture system audio. Grant it, then restart the app.',
    ),
  ).toBeInTheDocument()
  fireEvent.click(screen.getByRole('button', { name: 'Open System Settings' }))
  await waitFor(() => expect(api.openScreenRecordingSettings).toHaveBeenCalled())
})

test('model_load_error_links_to_settings', async () => {
  vi.mocked(api.listCourses).mockResolvedValue(['Algorithms 101'])
  vi.mocked(api.startSession).mockRejectedValueOnce(
    'Whisper model failed to load — download it again in Settings',
  )
  render(<Main />)
  fireEvent.click(screen.getByRole('button', { name: 'Start' }))
  fireEvent.click(await screen.findByRole('button', { name: 'Algorithms 101' }))
  expect(await screen.findByRole('link', { name: 'Open Settings' })).toHaveAttribute(
    'href',
    '#/settings',
  )
})

test('retry_button_disappears_when_a_new_session_starts', async () => {
  vi.mocked(api.listCourses).mockResolvedValue(['Algorithms 101'])
  render(<Main />)
  send({ type: 'SummaryFailed', error: 'network error' })
  expect(screen.getByRole('button', { name: 'Retry summary' })).toBeInTheDocument()
  fireEvent.click(screen.getByRole('button', { name: 'Start' }))
  fireEvent.click(await screen.findByRole('button', { name: 'Algorithms 101' }))
  await screen.findByRole('button', { name: 'Stop' })
  expect(screen.queryByRole('button', { name: 'Retry summary' })).toBeNull()
})
