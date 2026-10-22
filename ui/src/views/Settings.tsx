import { useCallback, useEffect, useState, type FormEvent } from 'react'
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

const NOTICE =
  'Transcript excerpts, the full transcript for summaries, and screenshots (if enabled) are sent to this provider. Audio never leaves this computer.'

const cleanLines = (lines: string[]) =>
  lines.map((line) => line.trim()).filter((line) => line !== '')

function Settings() {
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
      <main className="page">
        Loading…{error && <span className="error">{error}</span>}
      </main>
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
    <main className="page">
      <header className="bar">
        <h1>Settings</h1>
        <a href="#/">Back</a>
      </header>

      <form onSubmit={(event) => void save(event)}>
        <fieldset>
          <legend>Provider</legend>
          <label className="field">
            <span>API base URL</span>
            <input
              value={settings.api_base_url}
              onChange={(event) => patch({ api_base_url: event.target.value })}
            />
          </label>
          <label className="field">
            <span>Model</span>
            <input
              value={settings.model}
              onChange={(event) => patch({ model: event.target.value })}
            />
          </label>
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
          <label className="field">
            <span>API key</span>
            <input
              type="password"
              value={key}
              onChange={(event) => setKey(event.target.value)}
            />
          </label>
          <div className="actions">
            {keySaved ? <span className="muted">Key saved</span> : <span className="muted">No key saved</span>}
            {keySaved && <button type="button" onClick={() => void removeKey()}>Remove key</button>}
          </div>
          <p className="muted">{NOTICE}</p>
        </fieldset>

        <fieldset>
          <legend>Speech</legend>
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
          <div className="actions">
            <button type="button" onClick={() => void download()}>Download</button>
            {model && (
              <span className="muted">
                Model {model.model} {model.installed ? 'installed' : 'not installed'}
              </span>
            )}
          </div>
          {progress && (
            <div className="actions">
              <progress max={progress.total || 1} value={progress.done} />
              <span className="muted">
                {progress.total > 0 ? `${Math.round((100 * progress.done) / progress.total)}%` : ''}
              </span>
            </div>
          )}
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
          <label className="field">
            <span>Hotkey</span>
            <input
              value={settings.hotkey}
              onChange={(event) => patch({ hotkey: event.target.value })}
            />
          </label>
        </fieldset>

        <fieldset>
          <legend>Hints</legend>
          <label className="field">
            <span>Name variants</span>
            <textarea
              value={settings.name_variants.join('\n')}
              onChange={(event) => patch({ name_variants: event.target.value.split('\n') })}
            />
          </label>
          <label className="field">
            <span>Keywords</span>
            <textarea
              value={settings.keywords.join('\n')}
              onChange={(event) => patch({ keywords: event.target.value.split('\n') })}
            />
          </label>
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
        </fieldset>

        <fieldset>
          <legend>Screenshot</legend>
          <label className="field">
            <span>Screenshot mode</span>
            <select
              value={settings.screenshot_mode}
              onChange={(event) => patch({ screenshot_mode: event.target.value as ScreenshotMode })}
            >
              <option value="off">Off</option>
              <option value="region">Region</option>
              <option value="full">Full</option>
            </select>
          </label>
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
          <div className="actions">
            <button type="button" onClick={() => void pickRegion()}>Select region</button>
            <span className="muted">
              {settings.region
                ? `Region: ${settings.region.x}, ${settings.region.y} ${settings.region.width}×${settings.region.height}`
                : 'No region selected'}
            </span>
          </div>
        </fieldset>

        <fieldset>
          <legend>Notes</legend>
          <label className="field">
            <span>Notes folder</span>
            <input readOnly value={settings.notes_dir ?? ''} />
          </label>
          <button type="button" onClick={() => void chooseFolder()}>Choose…</button>
        </fieldset>

        <div className="actions">
          <button type="submit">Save</button>
          {status && <span className="muted">{status}</span>}
          {error && <span className="error">{error}</span>}
        </div>
      </form>
    </main>
  )
}

export default Settings
