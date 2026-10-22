import { useCallback, useEffect, useRef, useState, type MouseEvent } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { regionSnapshot, saveRegion } from '../api'
import type { RegionSnapshot } from '../bindings/RegionSnapshot'

type Point = { x: number; y: number }
type Rect = { x: number; y: number; width: number; height: number }

function rectOf(a: Point, b: Point): Rect {
  return {
    x: Math.min(a.x, b.x),
    y: Math.min(a.y, b.y),
    width: Math.abs(a.x - b.x),
    height: Math.abs(a.y - b.y),
  }
}

/// Displayed pixels in the image map onto the monitor's physical pixels.
function toPhysical(rect: Rect, snapshot: RegionSnapshot, img: HTMLImageElement): Rect {
  const fx = snapshot.monitor_width / img.clientWidth
  const fy = snapshot.monitor_height / img.clientHeight
  return {
    x: Math.round(rect.x * fx),
    y: Math.round(rect.y * fy),
    width: Math.round(rect.width * fx),
    height: Math.round(rect.height * fy),
  }
}

function RegionPicker() {
  const [snapshot, setSnapshot] = useState<RegionSnapshot | null>(null)
  const [loadError, setLoadError] = useState('')
  const [drag, setDrag] = useState<{ start: Point; current: Point } | null>(null)
  const [error, setError] = useState('')
  const imgRef = useRef<HTMLImageElement>(null)
  const rect = drag ? rectOf(drag.start, drag.current) : null
  const canSave = rect !== null && rect.width >= 4 && rect.height >= 4

  useEffect(() => {
    let live = true
    regionSnapshot()
      .then((data) => {
        if (live) setSnapshot(data)
      })
      .catch((cause: unknown) => {
        if (live) setLoadError(String(cause))
      })
    return () => {
      live = false
    }
  }, [])

  // The window starts hidden so the capture cannot include it.
  useEffect(() => {
    if (snapshot || loadError) void getCurrentWindow().show()
  }, [snapshot, loadError])

  const save = useCallback(async () => {
    const img = imgRef.current
    if (!snapshot || !rect || !img || !canSave || !img.clientWidth || !img.clientHeight) return
    setError('')
    try {
      await saveRegion(toPhysical(rect, snapshot, img))
    } catch (cause) {
      setError(String(cause))
    }
  }, [snapshot, rect, canSave])

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') void getCurrentWindow().close()
      if (event.key === 'Enter' && !(document.activeElement instanceof HTMLButtonElement)) {
        void save()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [save])

  const close = () => void getCurrentWindow().close()

  // Coordinates stay relative to the image, clamped to its edges.
  const point = (event: MouseEvent): Point => {
    const img = imgRef.current
    if (!img) return { x: 0, y: 0 }
    const box = img.getBoundingClientRect()
    return {
      x: Math.min(Math.max(event.clientX - box.left, 0), img.clientWidth),
      y: Math.min(Math.max(event.clientY - box.top, 0), img.clientHeight),
    }
  }

  if (loadError) {
    return (
      <div className="region-screen">
        <div className="region-bar">
          <span className="error">{loadError}</span>
          <button onClick={close}>Close</button>
        </div>
      </div>
    )
  }

  if (!snapshot) {
    return (
      <div className="region-screen">
        <div className="region-bar">
          <span>Taking a screenshot…</span>
          <button onClick={close}>Cancel</button>
        </div>
      </div>
    )
  }

  return (
    <div className="region-screen">
      <div className="region-bar">
        <span>
          {rect
            ? `${Math.round(rect.width)} × ${Math.round(rect.height)}`
            : 'Drag a rectangle on the image. Enter to save, Esc to cancel.'}
        </span>
        {error && <span className="error">{error}</span>}
        <button onClick={() => void save()} disabled={!canSave}>
          Save
        </button>
        <button onClick={close}>Cancel</button>
      </div>
      <div
        className="region-stage"
        onMouseDown={(event) => {
          if (event.target !== imgRef.current) return
          const pressed = point(event)
          setDrag({ start: pressed, current: pressed })
        }}
        onMouseMove={(event) => {
          if (drag) setDrag({ ...drag, current: point(event) })
        }}
        onMouseUp={(event) => {
          if (drag) setDrag({ ...drag, current: point(event) })
        }}
      >
        <img
          ref={imgRef}
          alt="Screenshot"
          src={`data:${snapshot.media_type};base64,${snapshot.image_base64}`}
        />
        {rect && (
          <div
            className="region-box"
            style={{
              left: rect.x,
              top: rect.y,
              width: rect.width,
              height: rect.height,
            }}
          />
        )}
      </div>
    </div>
  )
}

export default RegionPicker
