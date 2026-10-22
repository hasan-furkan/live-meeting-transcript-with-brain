import { useCallback, useEffect, useState, type MouseEvent } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { saveRegion } from '../api'

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

function RegionPicker() {
  const [start, setStart] = useState<Point | null>(null)
  const [current, setCurrent] = useState<Point | null>(null)
  const rect = start && current ? rectOf(start, current) : null

  useEffect(() => {
    document.documentElement.classList.add('transparent')
    document.body.classList.add('transparent')
    return () => {
      document.documentElement.classList.remove('transparent')
      document.body.classList.remove('transparent')
    }
  }, [])

  const save = useCallback(async () => {
    if (!rect || rect.width < 1 || rect.height < 1) return
    const scale = window.devicePixelRatio
    try {
      await saveRegion({
        x: Math.round(rect.x * scale),
        y: Math.round(rect.y * scale),
        width: Math.round(rect.width * scale),
        height: Math.round(rect.height * scale),
      })
    } catch {
      // The window stays open; the reason is shown in the main window.
    }
  }, [rect])

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') void getCurrentWindow().close()
      if (event.key === 'Enter') void save()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [save])

  const point = (event: MouseEvent) => ({ x: event.clientX, y: event.clientY })

  return (
    <div className="region-screen">
      <div
        className="region-overlay"
        onMouseDown={(event) => {
          const pressed = point(event)
          setStart(pressed)
          setCurrent(pressed)
        }}
        onMouseMove={(event) => {
          if (start) setCurrent(point(event))
        }}
        onMouseUp={(event) => {
          if (start) setCurrent(point(event))
        }}
      >
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
      <div className="region-bar" onMouseDown={(event) => event.stopPropagation()}>
        <span>Drag to select a region. Enter to save, Esc to cancel.</span>
        {rect && (
          <span>
            {Math.round(rect.width)} × {Math.round(rect.height)}
          </span>
        )}
        <button onClick={() => void save()} disabled={!rect}>
          Save
        </button>
        <button onClick={() => void getCurrentWindow().close()}>Cancel</button>
      </div>
    </div>
  )
}

export default RegionPicker
