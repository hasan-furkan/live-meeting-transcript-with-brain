import { useEffect, useState } from 'react'
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
import CoursePicker from '../components/CoursePicker'

const SCREEN_RECORDING_ERROR = 'Screen Recording permission required'

type Line = { time: string; text: string }
type Card = { id: number; time: string; analysis?: Analysis; error?: string; reason?: string }
type Summary = { path?: string; error?: string }

function Main() {
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
          fillCard(event.id, { time: event.time, analysis: event.analysis })
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

  return (
    <main className="page">
      <header className="bar">
        <h1>Live Meeting Assistant</h1>
        <a href="#/settings">Settings</a>
      </header>

      <div className="actions">
        {running ? (
          <button onClick={() => void stop()}>Stop</button>
        ) : (
          <button onClick={() => setPicking(true)}>Start</button>
        )}
        <button onClick={() => void ask()} disabled={!running}>
          Ask now
        </button>
        {running && <span className="muted">Recording: {course}</span>}
      </div>

      {picking && (
        <CoursePicker
          error={startError}
          onSubmit={(name) => void start(name)}
          onCancel={() => setPicking(false)}
        />
      )}

      {startError === SCREEN_RECORDING_ERROR && (
        <section className="error" aria-label="Screen Recording permission">
          <p>
            macOS needs Screen Recording permission to capture system audio. Grant it, then
            restart the app.
          </p>
          <button onClick={() => void openScreenRecordingSettings()}>Open System Settings</button>
        </section>
      )}

      <section>
        <h2>Transcript</h2>
        <ol className="transcript">
          {lines.map((line, index) => (
            <li key={`${line.time}-${index}`}>
              <span className="muted">{line.time}</span> {line.text}
            </li>
          ))}
        </ol>
        {partial && <p className="partial">{partial}</p>}
      </section>

      <section>
        <h2>Analysis</h2>
        <ul className="cards">
          {cards.map((card) => (
            <li key={card.id} className="card" data-testid="analysis-card">
              {card.analysis ? (
                <>
                  <div className="muted">{card.time}</div>
                  <p>{card.analysis.question || card.analysis.raw}</p>
                  <p>
                    <strong>Answer:</strong> {card.analysis.suggested_answer}
                  </p>
                  {card.analysis.explanation && (
                    <p className="muted">{card.analysis.explanation}</p>
                  )}
                  {card.analysis.confidence && (
                    <span className="badge">{card.analysis.confidence}</span>
                  )}
                </>
              ) : card.error ? (
                <p className="error">Analysis failed: {card.error}</p>
              ) : (
                <p className="muted">Analyzing… ({card.reason})</p>
              )}
            </li>
          ))}
        </ul>
      </section>

      <footer className="status">
        <span>{backend === 'Cpu' ? 'CPU mode — slow' : backend === 'Gpu' ? 'GPU' : 'Backend: —'}</span>
        <meter aria-label="Audio level" min={0} max={0.2} value={rms} />
        {noAudio && <span className="warn">No audio detected — check the output device</span>}
        {error && <span className="error">{error}</span>}
        {summary?.path && <span>Notes saved: {summary.path}</span>}
        {summary?.error && (
          <span className="error">
            Summary failed: {summary.error}{' '}
            <button onClick={() => void retry()}>Retry summary</button>
          </span>
        )}
      </footer>
    </main>
  )
}

export default Main
