import { useEffect, useState } from 'react'
import { listCourses } from '../api'

type Props = {
  error: string | null
  onSubmit: (course: string) => void
  onCancel: () => void
}

function CoursePicker({ error, onSubmit, onCancel }: Props) {
  const [courses, setCourses] = useState<string[]>([])
  const [loadError, setLoadError] = useState('')
  const [name, setName] = useState('')

  useEffect(() => {
    let active = true
    listCourses().then(
      (list) => {
        if (active) setCourses(list)
      },
      (cause: unknown) => {
        if (active) setLoadError(String(cause))
      },
    )
    return () => {
      active = false
    }
  }, [])

  const submit = () => {
    const course = name.trim()
    if (course) onSubmit(course)
  }

  return (
    <section className="picker" aria-label="Choose course">
      <h2>Choose course</h2>
      <ul>
        {courses.map((course) => (
          <li key={course}>
            <button onClick={() => onSubmit(course)}>{course}</button>
          </li>
        ))}
      </ul>
      <label className="field">
        <span>New course…</span>
        <input
          aria-label="New course"
          value={name}
          onChange={(event) => setName(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') submit()
          }}
        />
      </label>
      <div className="actions">
        <button onClick={submit} disabled={name.trim() === ''}>
          OK
        </button>
        <button onClick={onCancel}>Cancel</button>
      </div>
      {loadError && <p className="error">{loadError}</p>}
      {error && (
        <p className="error">
          {error} {error.includes('Settings') && <a href="#/settings">Open Settings</a>}
        </p>
      )}
    </section>
  )
}

export default CoursePicker
