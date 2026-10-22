import { useCallback, useEffect, useState, type FormEvent, type ReactNode } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import {
  downloadModel,
  getSettings,
  hasApiKey,
  listMonitors,
  listOutputDevices,
  modelStatus,
  onModelProgress,
  openRegionPicker,
  saveSettings,
  setApiKey,
} from '../api'
import type { ModelProgress } from '../bindings/ModelProgress'
import type { ModelStatus } from '../bindings/ModelStatus'
import type { ScreenshotMode } from '../bindings/ScreenshotMode'
import type { Settings as SettingsData } from '../bindings/Settings'
import Header, { type SessionStatus } from '../components/Header'

const NOTICE =
  'Transcript excerpts, the full transcript for summaries, and screenshots (if enabled) are sent to this provider. Audio never leaves this computer.'

const cleanLines = (lines: string[]) =>
  lines.map((line) => line.trim()).filter((line) => line !== '')

const MODES: { value: ScreenshotMode; label: string }[] = [
  { value: 'off', label: 'Off' },
  { value: 'region', label: 'Region' },
  { value: 'full', label: 'Full' },
]

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
    <section className="card section">
      <h2>{title}</h2>
      <p className="section-desc">{description}</p>
      {children}
    </section>
  )
}

function Settings({ session }: { session?: SessionStatus }) {
  const [settings, setSettings] = useState<SettingsData | null>(null)
  const [key, setKey] = useState('')
  const [keySaved, setKeySaved] = useState(false)
  const [monitors, setMonitors] = useState<string[]>([])
  const [devices, setDevices] = useState<string[]>([])
  const [model, setModel] = useState<ModelStatus | null>(null)
  const [progress, setProgress] = useState<ModelProgress | null>(null)
  const [status, setStatus] = useState('')
  const [error, setError] = useState('')

  const refreshRegion = useCallback(() => {
    void getSettings().then(
      (fresh) => setSettings((current) => (current ? { ...current, region: fresh.region } : current)),
      () => {},
    )
  }, [])

  useEffect(() => {
    void getSettings().then(setSettings, (cause: unknown) => setError(String(cause)))
    // The region picker window closes after saving; re-read only the region so unsaved edits survive.
    window.addEventListener('focus', refreshRegion)
    void hasApiKey().then(setKeySaved, () => {})
    void listMonitors().then(setMonitors, () => {})
    void listOutputDevices().then(setDevices, () => {})
    void modelStatus().then(setModel, () => {})
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
    }
  }, [refreshRegion])

  if (!settings) {
    return (
      <div className="app">
        <Header active="settings" status={session} />
        <main className="page">
          Loading…{error && <span className="error">{error}</span>}
        </main>
      </div>
    )
  }

  const patch = (change: Partial<SettingsData>) =>
    setSettings((current) => (current ? { ...current, ...change } : current))

  const save = async (event: FormEvent) => {
    event.preventDefault()
    setError('')
    setStatus('')
    const cleaned: SettingsData = {
      ...settings,
      name_variants: cleanLines(settings.name_variants),
      keywords: cleanLines(settings.keywords),
    }
    try {
      await saveSettings(cleaned)
      setSettings(cleaned)
      if (key.trim() !== '') {
        await setApiKey(key)
        setKey('')
        setKeySaved(true)
      }
      setStatus('Saved')
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

  return (
    <div className="app">
      <Header active="settings" status={session} />

      <main className="page settings">
        <div className="page-head">
          <h1>Settings</h1>
          <p>Changes apply immediately, also during a running session.</p>
        </div>

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
            <small className="muted">One per line. Changes apply immediately.</small>
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

          <Section title="Notes folder" description="Where the session summary is written.">
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
            <label className="field">
              <span>Monitor</span>
              <select
                value={settings.monitor ?? ''}
                onChange={(event) => patch({ monitor: event.target.value || null })}
              >
                <option value="">Default</option>
                {monitors.map((monitor) => (
                  <option key={monitor} value={monitor}>{monitor}</option>
                ))}
              </select>
            </label>
            <div className="row">
              <button type="button" onClick={() => void pickRegion()}>Select region</button>
              <span className="muted">
                {settings.region
                  ? `Region: ${settings.region.x}, ${settings.region.y} ${settings.region.width}×${settings.region.height}`
                  : 'No region selected'}
              </span>
            </div>
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
              <label className="field">
                <span>Audio device</span>
                <select
                  value={settings.audio_device ?? ''}
                  onChange={(event) => patch({ audio_device: event.target.value || null })}
                >
                  <option value="">Default</option>
                  {devices.map((device) => (
                    <option key={device} value={device}>{device}</option>
                  ))}
                </select>
              </label>
            </div>
          </Section>

          <div className="savebar">
            <span className="savebar-msg">
              {status && <span className="muted">{status}</span>}
              {error && <span className="error">{error}</span>}
            </span>
            <button type="submit" className="primary">Save</button>
          </div>
        </form>
      </main>
    </div>
  )
}

export default Settings
