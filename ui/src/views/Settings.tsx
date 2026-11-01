import { useCallback, useEffect, useRef, useState, type FormEvent, type ReactNode } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import {
  backgroundCaptureHint,
  downloadModel,
  getSettings,
  hasApiKey,
  listMonitors,
  listOutputDevices,
  listWindows,
  modelStatus,
  onModelProgress,
  openRegionPicker,
  saveSettings,
  setApiKey,
} from '../api'
import type { BackgroundHint } from '../bindings/BackgroundHint'
import type { ModelProgress } from '../bindings/ModelProgress'
import type { ModelStatus } from '../bindings/ModelStatus'
import type { ScreenshotMode } from '../bindings/ScreenshotMode'
import type { Settings as SettingsData } from '../bindings/Settings'
import type { WindowList } from '../bindings/WindowList'
import type { WindowTarget } from '../bindings/WindowTarget'
import Header, { type SessionStatus } from '../components/Header'
import Select, { type SelectOption } from '../components/Select'

const NOTICE =
  'Transcript excerpts, the full transcript for summaries, and screenshots (if enabled) are sent to this provider. Audio never leaves this computer.'

const cleanLines = (lines: string[]) =>
  lines.map((line) => line.trim()).filter((line) => line !== '')

const MODES: { value: ScreenshotMode; label: string }[] = [
  { value: 'off', label: 'Off' },
  { value: 'region', label: 'Region' },
  { value: 'full', label: 'Full' },
  { value: 'window', label: 'Window' },
]

// The id keeps two windows with the same app and title apart.
const windowKey = (target: WindowTarget) =>
  target.id === null ? JSON.stringify([target.app, target.title]) : String(target.id)
const windowLabel = (target: WindowTarget) => `${target.app} — ${target.title}`

function Section({
  title,
  description,
  children,
}: {
  title: string
  description: string
  children: ReactNode
}) {
  return (
    <section className="section">
      <div className="section-head">
        <h2>{title}</h2>
        <p className="section-desc">{description}</p>
      </div>
      <div className="section-body">{children}</div>
    </section>
  )
}

function Settings({ session }: { session?: SessionStatus }) {
  const [settings, setSettings] = useState<SettingsData | null>(null)
  const [saved, setSaved] = useState<SettingsData | null>(null)
  const [justSaved, setJustSaved] = useState(false)
  const savedTimer = useRef<number | undefined>(undefined)
  const [key, setKey] = useState('')
  const [keySaved, setKeySaved] = useState(false)
  const [monitors, setMonitors] = useState<string[]>([])
  const [windowList, setWindowList] = useState<WindowList>({ windows: [], current: null })
  const [windowsLoaded, setWindowsLoaded] = useState(false)
  const [windowsError, setWindowsError] = useState('')
  // Not saved in settings: it only widens the picker.
  const [showHiddenWindows, setShowHiddenWindows] = useState(false)
  const [devices, setDevices] = useState<string[]>([])
  const [model, setModel] = useState<ModelStatus | null>(null)
  const [progress, setProgress] = useState<ModelProgress | null>(null)
  const [hint, setHint] = useState<BackgroundHint | null>(null)
  const [copyState, setCopyState] = useState<'idle' | 'copied' | 'failed'>('idle')
  const copyTimer = useRef<number | undefined>(undefined)
  const [status, setStatus] = useState('')
  const [error, setError] = useState('')

  const refreshRegion = useCallback(() => {
    void getSettings().then(
      (fresh) => {
        setSettings((current) => (current ? { ...current, region: fresh.region } : current))
        setSaved((current) => (current ? { ...current, region: fresh.region } : current))
      },
      () => {},
    )
  }, [])

  // The saved choice, so the backend can say which live window it resolves to now.
  const savedWindow = settings?.window ?? null

  // Only the newest request may write: an older reply that arrives late is stale.
  const windowsRequest = useRef(0)

  const refreshWindows = useCallback(
    (hidden: boolean = showHiddenWindows) => {
      const request = (windowsRequest.current += 1)
      void listWindows(hidden, savedWindow).then(
        (list) => {
          if (request !== windowsRequest.current) return
          setWindowList(list)
          setWindowsLoaded(true)
          setWindowsError('')
        },
        // A failed background refresh keeps the last good list.
        (cause: unknown) => {
          if (request !== windowsRequest.current) return
          setWindowsError(String(cause))
        },
      )
    },
    [savedWindow, showHiddenWindows],
  )

  useEffect(() => {
    const loaded = (fresh: SettingsData) => {
      setSettings(fresh)
      setSaved(fresh)
    }
    void getSettings().then(loaded, (cause: unknown) => setError(String(cause)))
    // The region picker window closes after saving; re-read only the region so unsaved edits survive.
    window.addEventListener('focus', refreshRegion)
    void hasApiKey().then(setKeySaved, () => {})
    void listMonitors().then(setMonitors, () => {})
    void listOutputDevices().then(setDevices, () => {})
    void modelStatus().then(setModel, () => {})
    // Optional help: if the command cannot be fetched, the section is simply absent.
    void backgroundCaptureHint().then(setHint, () => {})
    let active = true
    let unlisten: (() => void) | undefined
    void onModelProgress((value) => setProgress(value)).then((stop) => {
      if (active) unlisten = stop
      else stop()
    })
    return () => {
      active = false
      unlisten?.()
      window.removeEventListener('focus', refreshRegion)
      window.clearTimeout(savedTimer.current)
      window.clearTimeout(copyTimer.current)
    }
  }, [refreshRegion])

  // The picker follows the windows that exist right now: on open, on focus, and while Window mode runs.
  const settingsLoaded = settings !== null
  const screenshotMode = settings?.screenshot_mode
  useEffect(() => {
    if (!settingsLoaded) return
    refreshWindows()
    const onFocus = () => refreshWindows()
    window.addEventListener('focus', onFocus)
    const timer =
      screenshotMode === 'window' ? window.setInterval(() => refreshWindows(), 5000) : undefined
    return () => {
      window.removeEventListener('focus', onFocus)
      window.clearInterval(timer)
    }
  }, [settingsLoaded, screenshotMode, refreshWindows])

  if (!settings) {
    return (
      <div className="app">
        <div className="backdrop" aria-hidden="true" />
        <Header active="settings" status={session} />
        <main className="page">
          Loading…{error && <span className="error">{error}</span>}
        </main>
      </div>
    )
  }

  const patch = (change: Partial<SettingsData>) =>
    setSettings((current) => (current ? { ...current, ...change } : current))

  const dirty = saved !== null && JSON.stringify(settings) !== JSON.stringify(saved)

  const save = async (event: FormEvent) => {
    event.preventDefault()
    setError('')
    setStatus('')
    setJustSaved(false)
    const cleaned: SettingsData = {
      ...settings,
      name_variants: cleanLines(settings.name_variants),
      keywords: cleanLines(settings.keywords),
    }
    try {
      await saveSettings(cleaned)
      setSettings(cleaned)
      setSaved(cleaned)
      if (key.trim() !== '') {
        await setApiKey(key)
        setKey('')
        setKeySaved(true)
      }
      window.clearTimeout(savedTimer.current)
      setJustSaved(true)
      savedTimer.current = window.setTimeout(() => setJustSaved(false), 3000)
    } catch (cause) {
      setError(String(cause))
    }
  }

  const removeKey = async () => {
    setError('')
    setStatus('')
    try {
      await setApiKey('')
      setKeySaved(false)
      setStatus('Key removed')
    } catch (cause) {
      setError(String(cause))
    }
  }

  const download = async () => {
    setError('')
    setStatus('')
    try {
      await downloadModel()
      setProgress(null)
      setModel(await modelStatus())
    } catch (cause) {
      setError(String(cause))
    }
  }

  const chooseFolder = async () => {
    try {
      const dir = await open({ directory: true })
      if (typeof dir === 'string') patch({ notes_dir: dir })
    } catch (cause) {
      setError(String(cause))
    }
  }

  const pickRegion = async () => {
    try {
      await openRegionPicker()
    } catch (cause) {
      setError(String(cause))
    }
  }

  const copyHint = async () => {
    if (!hint) return
    try {
      await navigator.clipboard.writeText(hint.command)
      setCopyState('copied')
    } catch {
      setCopyState('failed')
    }
    window.clearTimeout(copyTimer.current)
    copyTimer.current = window.setTimeout(() => setCopyState('idle'), 2000)
  }

  const copyLabel =
    copyState === 'copied' ? 'Copied' : copyState === 'failed' ? 'Copy failed' : 'Copy'

  const windowOptions: SelectOption[] = [
    { value: '', label: 'Choose a window…' },
    ...windowList.windows.map((target) => {
      const suffix = ` - ${target.app}`
      const secondary = target.title.endsWith(suffix)
        ? target.title.slice(0, -suffix.length)
        : target.title
      return {
        value: windowKey(target),
        label: windowLabel(target),
        primary: target.app,
        secondary: secondary === '' ? undefined : secondary,
      }
    }),
  ]

  const monitorOptions: SelectOption[] = [
    { value: '', label: 'Default' },
    ...monitors.map((monitor) => ({ value: monitor, label: monitor })),
  ]

  const audioOptions: SelectOption[] = [
    { value: '', label: 'Default' },
    ...devices.map((device) => ({ value: device, label: device })),
  ]

  return (
    <div className="app">
      <div className="backdrop" aria-hidden="true" />
      <Header active="settings" status={session} />

      <main className="page settings">
        <header className="page-head">
          <span className="page-label">Preferences</span>
          <h1>Settings</h1>
          <p>Press Save (or Enter) to apply. Saved trigger changes also apply to a running session.</p>
        </header>

        <form className="settings-form" onSubmit={(event) => void save(event)}>
          <Section
            title="AI provider"
            description="The model that answers questions during the session."
          >
            <label className="field">
              <span>API base URL</span>
              <input
                value={settings.api_base_url}
                onChange={(event) => patch({ api_base_url: event.target.value })}
              />
            </label>
            <label className="field">
              <span>API key</span>
              <input
                type="password"
                value={key}
                onChange={(event) => setKey(event.target.value)}
              />
            </label>
            <div className="row">
              {keySaved ? (
                <span className="muted">Key saved</span>
              ) : (
                <span className="muted">No key saved</span>
              )}
              {keySaved && (
                <button type="button" onClick={() => void removeKey()}>
                  Remove key
                </button>
              )}
            </div>
            <label className="field">
              <span>Model</span>
              <input
                value={settings.model}
                onChange={(event) => patch({ model: event.target.value })}
              />
            </label>
            <p className="notice">{NOTICE}</p>
          </Section>

          <Section
            title="Speech recognition"
            description="Local Whisper transcription. Audio never leaves this computer."
          >
            <div className="grid-2">
              <label className="field">
                <span>Whisper model</span>
                <input
                  value={settings.whisper_model}
                  onChange={(event) => patch({ whisper_model: event.target.value })}
                />
              </label>
              <label className="field">
                <span>Language</span>
                <input
                  value={settings.language}
                  onChange={(event) => patch({ language: event.target.value })}
                />
              </label>
            </div>
            <div className="row">
              <button type="button" onClick={() => void download()}>
                Download
              </button>
              {model && (
                <span className="muted">
                  Model <span className="mono">{model.model}</span>{' '}
                  {model.installed ? 'installed' : 'not installed'}
                </span>
              )}
            </div>
            {progress && (
              <div className="row">
                <progress max={progress.total || 1} value={progress.done} />
                <span className="muted mono">
                  {progress.total > 0 ? `${Math.round((100 * progress.done) / progress.total)}%` : ''}
                </span>
              </div>
            )}
          </Section>

          <Section
            title="Triggers"
            description="When the assistant speaks up during a session."
          >
            <label className="field">
              <span>Name variants</span>
              <textarea
                value={settings.name_variants.join('\n')}
                onChange={(event) => patch({ name_variants: event.target.value.split('\n') })}
              />
            </label>
            <small className="muted">One per line. After saving, also applies to a running session.</small>
            <label className="field">
              <span>Keywords</span>
              <textarea
                value={settings.keywords.join('\n')}
                onChange={(event) => patch({ keywords: event.target.value.split('\n') })}
              />
            </label>
            <div className="toggles">
              <label className="field check">
                <input
                  type="checkbox"
                  checked={settings.general_questions}
                  onChange={(event) => patch({ general_questions: event.target.checked })}
                />
                <span>General questions</span>
              </label>
              <label className="field check">
                <input
                  type="checkbox"
                  checked={settings.name_bias}
                  onChange={(event) => patch({ name_bias: event.target.checked })}
                />
                <span>Name bias</span>
              </label>
            </div>
            <label className="field">
              <span>Explanation language</span>
              <input
                value={settings.explanation_language}
                maxLength={40}
                onChange={(event) => patch({ explanation_language: event.target.value })}
              />
              <small className="muted">
                Optional. A short explanation in this language is added under each suggested answer
                (e.g. Turkish, Deutsch). Leave empty to turn it off.
              </small>
            </label>
          </Section>

          <Section title="Notes" description="Where the session summary is written.">
            <label className="field">
              <span>Notes folder</span>
              <input readOnly value={settings.notes_dir ?? ''} />
            </label>
            <div className="row">
              <button type="button" onClick={() => void chooseFolder()}>
                Choose…
              </button>
            </div>
          </Section>

          <Section
            title="Screenshots"
            description="Optionally attach the shared screen to each analysis."
          >
            <div className="field">
              <span id="screenshot-mode-label">Screenshot mode</span>
              <div className="segmented" role="group" aria-labelledby="screenshot-mode-label">
                {MODES.map((mode) => (
                  <button
                    key={mode.value}
                    type="button"
                    className={settings.screenshot_mode === mode.value ? 'seg active' : 'seg'}
                    aria-pressed={settings.screenshot_mode === mode.value}
                    onClick={() => patch({ screenshot_mode: mode.value })}
                  >
                    {mode.label}
                  </button>
                ))}
              </div>
            </div>
            {settings.screenshot_mode === 'window' ? (
              <div className="field">
                <label htmlFor="window-select">Window</label>
                <div className="row">
                  <Select
                    id="window-select"
                    value={windowList.current ? windowKey(windowList.current) : ''}
                    options={windowOptions}
                    onOpen={() => refreshWindows()}
                    onChange={(next) => {
                      const picked = windowList.windows.find(
                        (target) => windowKey(target) === next,
                      )
                      patch({ window: picked ?? null })
                    }}
                  />
                  <button type="button" onClick={() => refreshWindows()}>
                    Refresh
                  </button>
                </div>
                <p className="muted">
                  A browser window always shows its open tab. To keep one tab while you use others,
                  drag that tab out into its own window, then choose that window here.
                </p>
                <label className="field check">
                  <input
                    type="checkbox"
                    checked={showHiddenWindows}
                    onChange={(event) => {
                      setShowHiddenWindows(event.target.checked)
                      refreshWindows(event.target.checked)
                    }}
                  />
                  <span>Also show minimized windows and other desktops</span>
                </label>
                {savedWindow && windowsLoaded && windowList.current === null && (
                  <span className="muted">
                    The saved window is not open. Open it, or choose another.
                  </span>
                )}
                <span className="muted">
                  {windowsError ||
                    'If the title changes (e.g. a new tab) the app keeps following the same window.'}
                </span>
                {hint && (
                  <details className="hint">
                    <summary>Window comes back black?</summary>
                    <p className="muted">{hint.text}</p>
                    <code className="hint-command">{hint.command}</code>
                    <div className="row">
                      <button type="button" onClick={() => void copyHint()}>
                        {copyLabel}
                      </button>
                    </div>
                  </details>
                )}
              </div>
            ) : (
            <>
            <div className="field">
              <label htmlFor="monitor-select">Monitor</label>
              <Select
                id="monitor-select"
                value={settings.monitor ?? ''}
                options={monitorOptions}
                onChange={(next) => patch({ monitor: next === '' ? null : next })}
              />
            </div>
            <div className="row">
              <button type="button" onClick={() => void pickRegion()}>Select region</button>
              <span className="muted">
                {settings.region
                  ? `Region: ${settings.region.x}, ${settings.region.y} ${settings.region.width}×${settings.region.height}`
                  : 'No region selected'}
              </span>
            </div>
            </>
            )}
          </Section>

          <Section
            title="Hotkey & audio"
            description="Keyboard shortcut and the audio output device to listen to."
          >
            <div className="grid-2">
              <label className="field">
                <span>Hotkey</span>
                <input
                  className="mono"
                  value={settings.hotkey}
                  onChange={(event) => patch({ hotkey: event.target.value })}
                />
              </label>
              <div className="field">
                <label htmlFor="audio-select">Audio device</label>
                <Select
                  id="audio-select"
                  value={settings.audio_device ?? ''}
                  options={audioOptions}
                  onChange={(next) => patch({ audio_device: next === '' ? null : next })}
                />
              </div>
            </div>
          </Section>

          <div className="savebar">
            <span className="savebar-msg">
              {dirty && <span className="warn">Unsaved changes — press Save (or Enter)</span>}
              {justSaved && <span className="muted">Saved ✓</span>}
              {status && <span className="muted">{status}</span>}
              {error && <span className="error">{error}</span>}
            </span>
            <button type="submit" className={dirty ? 'primary' : undefined}>
              Save
            </button>
          </div>
        </form>
      </main>
    </div>
  )
}

export default Settings
