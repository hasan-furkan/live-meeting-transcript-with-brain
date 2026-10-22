import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, expect, test, vi } from 'vitest'
import type { SessionEvent } from '../bindings/SessionEvent'

vi.mock('../api', () => ({
  onSessionEvent: vi.fn(() => Promise.resolve(() => {})),
  listCourses: vi.fn(async () => []),
  startSession: vi.fn(async () => {}),
  stopSession: vi.fn(async () => {}),
  askNow: vi.fn(async () => {}),
  retrySummary: vi.fn(async () => {}),
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
    analysis: { question: 'Q1', suggested_answer: 'A1', confidence: 'high', raw: null },
  })
  send({ type: 'AnalysisStarted', id: 2, reason: 'question' })
  send({
    type: 'Analysis',
    id: 2,
    time: '10:01',
    analysis: { question: 'Q2', suggested_answer: 'A2', confidence: 'medium', raw: null },
  })
  send({ type: 'AnalysisStarted', id: 3, reason: 'question' })
  send({
    type: 'Analysis',
    id: 3,
    time: '10:02',
    analysis: { question: '', suggested_answer: 'A3', confidence: null, raw: 'raw text' },
  })
  const cards = screen.getAllByTestId('analysis-card')
  expect(cards[0]).toHaveTextContent('raw text')
  expect(cards[1]).toHaveTextContent('Q2')
  expect(cards[2]).toHaveTextContent('Q1')
  expect(cards[2]).toHaveTextContent('high')
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
