import { useEffect, useState } from 'react'
import Main from './views/Main'
import Settings from './views/Settings'
import RegionPicker from './views/RegionPicker'
import { IDLE, type SessionStatus } from './components/Header'

function currentRoute(): string {
  return window.location.hash || '#/'
}

function App() {
  const [route, setRoute] = useState(currentRoute)
  const [status, setStatus] = useState<SessionStatus>(IDLE)

  useEffect(() => {
    const onChange = () => setRoute(currentRoute())
    window.addEventListener('hashchange', onChange)
    return () => window.removeEventListener('hashchange', onChange)
  }, [])

  // The region picker lives in its own window; Main stays mounted elsewhere so a
  // trip to Settings does not drop live session state.
  if (route === '#/region') return <RegionPicker />
  return (
    <>
      <div hidden={route !== '#/'}>
        <Main onStatusChange={setStatus} />
      </div>
      {route === '#/settings' && <Settings session={status} />}
    </>
  )
}

export default App
