import { useEffect, useRef, useState } from 'react'
import {
  askNow,
  onSessionEvent,
  openScreenRecordingSettings,
  retrySummary,
  startSession,
  stopSession,
} from '../api'
import type { Analysis } from '../bindings/Analysis'
import type { Backend } from '../bindings/Backend'
import type { ScreenshotStatus } from '../bindings/ScreenshotStatus'
import CoursePicker from '../components/CoursePicker'
import Header, { type SessionStatus } from '../components/Header'

const SCREEN_RECORDING_ERROR = 'Screen Recording permission required'

type Line = { time: string; text: string }
type Card = {
  id: number
  time: string
  analysis?: Analysis
  screenshot?: ScreenshotStatus
  error?: string
  reason?: string
}
type Summary = { path?: string; error?: string }

const reasonLabel = (reason: string | undefined) => {
  if (!reason) return 'Manual'
  return reason.charAt(0).toUpperCase() + reason.slice(1)
}

function Main({ onStatusChange }: { onStatusChange?: (status: SessionStatus) => void }) {
  const [running, setRunning] = useState(false)
  const [course, setCourse] = useState('')
  const [picking, setPicking] = useState(false)
  const [startError, setStartError] = useState<string | null>(null)
  const [lines, setLines] = useState<Line[]>([])
  const [partial, setPartial] = useState('')
  const [cards, setCards] = useState<Card[]>([])
  const [backend, setBackend] = useState<Backend | null>(null)
  const [rms, setRms] = useState(0)
  const [noAudio, setNoAudio] = useState(false)
  const [error, setError] = useState('')
  const [summary, setSummary] = useState<Summary | null>(null)
  const transcriptRef = useRef<HTMLOListElement>(null)

  // The header also renders on Settings, where Main stays mounted but hidden.
  useEffect(() => {
    onStatusChange?.({ running, course })
  }, [running, course, onStatusChange])

  useEffect(() => {
    const list = transcriptRef.current
    if (list) list.scrollTop = list.scrollHeight
  }, [lines, partial])

  useEffect(() => {
    let active = true
    let unlisten: (() => void) | undefined
    const fillCard = (id: number, patch: Partial<Card>) =>
      setCards((cards) => cards.map((card) => (card.id === id ? { ...card, ...patch } : card)))

    void onSessionEvent((event) => {
      switch (event.type) {
        case 'Partial':
          setPartial(event.text)
          break
        case 'Line':
          setLines((lines) => [...lines, { time: event.time, text: event.text }].slice(-20))
          setPartial('')
          break
        case 'AnalysisStarted':
          setCards((cards) => [{ id: event.id, time: '', reason: event.reason }, ...cards].slice(0, 30))
          break
        case 'Analysis':
          fillCard(event.id, {
            time: event.time,
            analysis: event.analysis,
            screenshot: event.screenshot,
          })
          break
        case 'AnalysisFailed':
          fillCard(event.id, { error: event.error })
          break
        case 'Level':
          setRms(event.rms)
          if (event.rms >= 0.001) setNoAudio(false)
          break
        case 'NoAudio':
          setNoAudio(true)
          break
        case 'Backend':
          setBackend(event.backend)
          break
        case 'Error':
          setError(event.message)
          break
        case 'SummaryWritten':
          setSummary({ path: event.path })
          break
        case 'SummaryFailed':
          setSummary({ error: event.error })
          break
        default:
          break
      }
    }).then((stop) => {
      if (active) unlisten = stop
      else stop()
    })

    return () => {
      active = false
      unlisten?.()
    }
  }, [])

  const start = async (name: string) => {
    setStartError(null)
    setSummary(null)
    try {
      await startSession(name)
      setCourse(name)
      setRunning(true)
      setPicking(false)
    } catch (cause) {
      setStartError(String(cause))
    }
  }

  const stop = async () => {
    try {
      await stopSession()
    } catch (cause) {
      setError(String(cause))
    } finally {
      setRunning(false)
    }
  }

  const ask = async () => {
    try {
      await askNow()
    } catch (cause) {
      setError(String(cause))
    }
  }

  const retry = async () => {
    setSummary(null)
    try {
      await retrySummary()
    } catch (cause) {
      setSummary({ error: String(cause) })
    }
  }

  const engine = backend === 'Cpu' ? 'CPU mode — slow' : backend === 'Gpu' ? 'GPU' : '—'

  return (
    <div className="app">
      <Header active="live" status={{ running, course }} />

      <main className="page">
        <section className="stats" aria-label="Session overview">
          <div className="stat">
            <span className="stat-label">Session</span>
            <span className="stat-value mono">{course || '—'}</span>
          </div>
          <div className="stat">
            <span className="stat-label">Speech engine</span>
            <span className={backend === 'Cpu' ? 'stat-value mono warn' : 'stat-value mono'}>
              {engine}
            </span>
          </div>
          <div className="stat">
            <span className="stat-label">Audio level</span>
            <meter aria-label="Audio level" min={0} max={0.2} value={rms} />
          </div>
          <div className="stat">
            <span className="stat-label">Analyses</span>
            <span className="stat-value mono">{cards.length}</span>
          </div>
        </section>

        <div className="columns">
          <section className="panel">
            <div className="panel-head">
              <h2>Live transcript</h2>
              {running ? (
                <button onClick={() => void stop()}>Stop</button>
              ) : (
                <button className="primary" onClick={() => setPicking(true)}>
                  Start
                </button>
              )}
            </div>
            <ol className="transcript" ref={transcriptRef}>
              {lines.map((line, index) => (
                <li key={`${line.time}-${index}`}>
                  <span className="line-time">{line.time}</span>
                  <span className="line-text">{line.text}</span>
                </li>
              ))}
            </ol>
            {partial && <p className="partial">{partial}</p>}
          </section>

          <section className="panel">
            <div className="panel-head">
              <h2>Questions &amp; suggested answers</h2>
              <button onClick={() => void ask()} disabled={!running}>
                Ask now
              </button>
            </div>
            <ul className="cards">
              {cards.map((card) => (
                <li
                  key={card.id}
                  className={card.analysis || card.error ? 'card' : 'card pending'}
                  data-testid="analysis-card"
                >
                  <div className="card-top">
                    <span className="chip">{reasonLabel(card.reason)}</span>
                    {card.time && <span className="card-time">{card.time}</span>}
                  </div>
                  {card.analysis ? (
                    <>
                      <p className="question">{card.analysis.question || card.analysis.raw}</p>
                      <div>
                        <span className="answer-label">Suggested answer</span>
                        <p className="answer">{card.analysis.suggested_answer}</p>
                      </div>
                      {card.analysis.explanation && (
                        <p className="explanation">{card.analysis.explanation}</p>
                      )}
                      <div className="card-badges">
                        {card.analysis.confidence && (
                          <span className="badge">{card.analysis.confidence}</span>
                        )}
                        {card.screenshot === 'attached' && (
                          <span className="badge ok">📷 with screenshot</span>
                        )}
                        {card.screenshot === 'failed' && (
                          <span className="badge warn">📷 screenshot failed</span>
                        )}
                      </div>
                    </>
                  ) : card.error ? (
                    <p className="error">Analysis failed: {card.error}</p>
                  ) : (
                    <p className="muted">Analyzing…</p>
                  )}
                </li>
              ))}
            </ul>
          </section>
        </div>

        {startError === SCREEN_RECORDING_ERROR && (
          <section className="card notice-card" aria-label="Screen Recording permission">
            <p>
              macOS needs Screen Recording permission to capture system audio. Grant it, then
              restart the app.
            </p>
            <button onClick={() => void openScreenRecordingSettings()}>
              Open System Settings
            </button>
          </section>
        )}

        {picking && (
          <CoursePicker
            error={startError}
            onSubmit={(name) => void start(name)}
            onCancel={() => setPicking(false)}
          />
        )}
      </main>

      <footer className="statusbar">
        <div className="status-left">
          {noAudio && <span className="warn">No audio detected — check the output device</span>}
          {error && <span className="error">{error}</span>}
        </div>
        <div className="status-right">
          {summary?.path && (
            <span>
              Notes saved: <span className="path">{summary.path}</span>
            </span>
          )}
          {summary?.error && (
            <span className="error">
              Summary failed: {summary.error}{' '}
              <button onClick={() => void retry()}>Retry summary</button>
            </span>
          )}
        </div>
      </footer>
    </div>
  )
}

export default Main
