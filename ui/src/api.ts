import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { ModelProgress } from './bindings/ModelProgress'
import type { ModelStatus } from './bindings/ModelStatus'
import type { Region } from './bindings/Region'
import type { SessionEvent } from './bindings/SessionEvent'
import type { Settings } from './bindings/Settings'

export const getSettings = () => invoke<Settings>('get_settings')
export const saveSettings = (settings: Settings) => invoke('save_settings', { settings })
export const hasApiKey = () => invoke<boolean>('has_api_key')
export const setApiKey = (key: string) => invoke('set_api_key', { key })
export const listCourses = () => invoke<string[]>('list_courses')
export const startSession = (course: string) => invoke('start_session', { course })
export const stopSession = () => invoke('stop_session')
export const askNow = () => invoke('ask_now')
export const retrySummary = () => invoke('retry_summary')
export const modelStatus = () => invoke<ModelStatus>('model_status')
export const downloadModel = () => invoke('download_model')
export const listMonitors = () => invoke<string[]>('list_monitors')
export const listOutputDevices = () => invoke<string[]>('list_output_devices')
export const openRegionPicker = () => invoke('open_region_picker')
export const saveRegion = (region: Region) => invoke('save_region', { region })

export const onSessionEvent = (callback: (event: SessionEvent) => void): Promise<UnlistenFn> =>
  listen<SessionEvent>('session-event', (event) => callback(event.payload))

export const onModelProgress = (callback: (progress: ModelProgress) => void): Promise<UnlistenFn> =>
  listen<ModelProgress>('model-progress', (event) => callback(event.payload))
