import { useEffect, useRef, useState } from 'react'
import {
  askNow,
  captureAndAsk,
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
import Icon from '../components/Icon'

const SCREEN_RECORDING_ERROR = 'Screen Recording permission required'

type Line = { time: string; text: string }
type Card = {
  id: number
  time: string
  analysis?: Analysis
  screenshot?: ScreenshotStatus
  image?: string | null
  error?: string
  reason?: string
}
type Summary = { path?: string; error?: string }

/** -60 dBFS (silence) to 0 dBFS (full scale) mapped onto 0–100 %. */
const levelPercent = (rms: number) =>
  rms > 0 ? Math.min(100, Math.max(0, ((20 * Math.log10(rms) + 60) / 60) * 100)) : 0

const levelLabel = (rms: number) => (rms > 0 ? `${Math.round(20 * Math.log10(rms))} dB` : '—')

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
  const [saving, setSaving] = useState(false)
  const [saveHidden, setSaveHidden] = useState(false)
  const [elapsed, setElapsed] = useState(0)
  const [zoom, setZoom] = useState<string | null>(null)
  const transcriptRef = useRef<HTMLOListElement>(null)

  useEffect(() => {
    if (!zoom) return
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setZoom(null)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [zoom])

  // The header also renders on Settings, where Main stays mounted but hidden.
  useEffect(() => {
    onStatusChange?.({ running, course })
  }, [running, course, onStatusChange])

  useEffect(() => {
    const list = transcriptRef.current
    if (list) list.scrollTop = list.scrollHeight
  }, [lines, partial])

  // Ticks the elapsed-seconds counter while notes are being saved.
  useEffect(() => {
    if (!saving) return
    const timer = window.setInterval(() => setElapsed((value) => value + 1), 1000)
    return () => window.clearInterval(timer)
  }, [saving])

  useEffect(() => {
    let active = true
    let unlisten: (() => void) | undefined
    const fillCard = (id: number, patch: Partial<Card>) =>
      setCards((cards) => cards.map((card) => (card.id === id ? { ...card, ...patch } : card)))

    void onSessionEvent((event) => {
      switch (event.type) {
        // Recognized speech proves audio is arriving, whatever the level reads.
        case 'Partial':
          setPartial(event.text)
          setNoAudio(false)
          break
        case 'Line':
          setLines((lines) => [...lines, { time: event.time, text: event.text }].slice(-20))
          setPartial('')
          setNoAudio(false)
          break
        case 'AnalysisStarted':
          setCards((cards) => [{ id: event.id, time: '', reason: event.reason }, ...cards].slice(0, 30))
          break
        case 'Analysis':
          fillCard(event.id, {
            time: event.time,
            analysis: event.analysis,
            screenshot: event.screenshot,
            image: event.image,
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
          setSaving(false)
          break
        case 'SummaryFailed':
          setSummary({ error: event.error })
          setSaving(false)
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
    setSaveHidden(false)
    setSummary(null)
    setElapsed(0)
    setSaving(true)
    try {
      await stopSession()
    } catch (cause) {
      setError(String(cause))
      setSaving(false)
    } finally {
      setRunning(false)
    }
  }

  const ask = async (how: () => Promise<unknown>) => {
    try {
      await how()
    } catch (cause) {
      setError(String(cause))
    }
  }

  const retry = async () => {
    setSaveHidden(false)
    setSummary(null)
    setElapsed(0)
    setSaving(true)
    try {
      await retrySummary()
    } catch (cause) {
      setSummary({ error: String(cause) })
    } finally {
      setSaving(false)
    }
  }

  const engine = backend === 'Cpu' ? 'CPU mode — slow' : backend === 'Gpu' ? 'GPU' : '—'

  return (
    <div className="app">
      <div className="backdrop" aria-hidden="true" />
      <Header active="live" status={{ running, course }} />

      <main className="page live">
        <section className="session-band" aria-label="Session overview">
          <div className="session-main">
            <span className="band-label">Session</span>
            <h1>{course || 'No session running'}</h1>
            <p className="band-sentence">
              {running
                ? 'Transcribing system audio on this computer.'
                : 'Start a session to transcribe the meeting and get suggested answers.'}
            </p>
          </div>
          <div className="metrics">
            <div className="metric">
              <span className="metric-label">Speech engine</span>
              <span className={backend === 'Cpu' ? 'metric-value mono warn' : 'metric-value mono'}>
                {engine}
              </span>
            </div>
            <div className="metric">
              <span className="metric-label">Audio level</span>
              <div className="level-row">
                <div
                  className="level"
                  role="meter"
                  aria-label="Audio level"
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-valuenow={Math.round(levelPercent(rms))}
                  aria-valuetext={levelLabel(rms)}
                >
                  <div className="level-fill" style={{ width: `${levelPercent(rms)}%` }} />
                </div>
                <span className="level-db mono">{levelLabel(rms)}</span>
              </div>
            </div>
            <div className="metric">
              <span className="metric-label">Analyses</span>
              <span className="metric-value mono">{cards.length}</span>
            </div>
          </div>
          <div className="band-action">
            {running ? (
              <button className="danger" onClick={() => void stop()} disabled={saving}>
                <Icon name="stop" />
                Stop
              </button>
            ) : (
              <button className="primary" onClick={() => setPicking(true)} disabled={saving}>
                <Icon name="play" />
                Start
              </button>
            )}
          </div>
        </section>

        {startError === SCREEN_RECORDING_ERROR && (
          <section className="notice-card" aria-label="Screen Recording permission">
            <p>
              macOS needs Screen Recording permission to capture system audio. Grant it, then
              restart the app.
            </p>
            <button onClick={() => void openScreenRecordingSettings()}>
              Open System Settings
            </button>
          </section>
        )}

        <div className="columns">
          <section className="panel panel-answers">
            <div className="panel-head">
              <h2>Questions &amp; suggested answers</h2>
              <div className="head-actions">
                <button
                  className={running ? 'primary' : undefined}
                  onClick={() => void ask(askNow)}
                  disabled={!running}
                >
                  <Icon name="sparkle" />
                  Ask now
                </button>
                <button
                  onClick={() => void ask(captureAndAsk)}
                  disabled={!running}
                  title="Takes a screenshot of the selected region (or the whole screen) and asks about it"
                >
                  <Icon name="camera" />
                  Capture &amp; ask
                </button>
              </div>
            </div>
            <ul className="cards">
              {cards.length === 0 && (
                <li className="empty-state">
                  <h3>No answers yet</h3>
                  <p>
                    {running
                      ? 'Listening for questions. You can also press Ask now.'
                      : 'Questions you are asked will appear here with a suggested answer.'}
                  </p>
                </li>
              )}
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
                      {card.analysis.raw ? (
                        <>
                          <span className="answer-label">Unreadable reply from the model</span>
                          <p className="raw">{card.analysis.raw}</p>
                        </>
                      ) : (
                        <>
                          <p className="question">{card.analysis.question}</p>
                          <div>
                            <span className="answer-label">Suggested answer</span>
                            <p className="answer">{card.analysis.suggested_answer}</p>
                          </div>
                          {card.analysis.explanation && (
                            <p className="explanation">{card.analysis.explanation}</p>
                          )}
                        </>
                      )}
                      <div className="card-badges">
                        {card.analysis.confidence && (
                          <span className="badge">{card.analysis.confidence}</span>
                        )}
                        {card.screenshot === 'attached' && (
                          <span className="badge ok">
                            <Icon name="camera" size={16} />
                            with screenshot
                          </span>
                        )}
                        {card.screenshot === 'failed' && (
                          <span className="badge warn">
                            <Icon name="camera" size={16} />
                            screenshot failed
                          </span>
                        )}
                      </div>
                      {card.image && (
                        <button
                          type="button"
                          className="card-shot"
                          onClick={() => setZoom(card.image ?? null)}
                        >
                          <img src={card.image} alt="Screenshot sent with the question" />
                        </button>
                      )}
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

          <section className="panel panel-transcript">
            <div className="panel-head">
              <h2>Live transcript</h2>
            </div>
            <ol className="transcript" ref={transcriptRef}>
              {lines.map((line, index) => (
                <li key={`${line.time}-${index}`}>
                  <span className="line-time">{line.time}</span>
                  <span className="line-text">{line.text}</span>
                </li>
              ))}
              {lines.length === 0 && !partial && (
                <li className="empty-state">
                  <h3>Nothing transcribed yet</h3>
                  <p>
                    {running
                      ? 'Waiting for speech.'
                      : 'The transcript appears here when a session is running.'}
                  </p>
                </li>
              )}
              {partial && (
                <li className="partial-line">
                  <span className="line-time" />
                  <span className="line-text partial">{partial}</span>
                </li>
              )}
            </ol>
          </section>
        </div>

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

      {saving && !saveHidden && (
        <div className="saving-popup" role="status" aria-live="polite">
          <span className="spinner" aria-hidden="true" />
          <p className="saving-title">Saving your notes…</p>
          <p className="saving-detail">Writing the summary and transcript. You can keep working.</p>
          <p className="saving-elapsed mono">{elapsed}s</p>
          <button onClick={() => setSaveHidden(true)}>Hide</button>
        </div>
      )}

      {zoom && (
        <div className="overlay" role="dialog" aria-label="Screenshot" onClick={() => setZoom(null)}>
          <img src={zoom} alt="Screenshot sent with the question" />
          <button onClick={() => setZoom(null)}>Close</button>
        </div>
      )}
    </div>
  )
}

export default Main
