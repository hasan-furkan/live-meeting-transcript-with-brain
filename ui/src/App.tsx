import { useEffect, useState } from 'react'
import Main from './views/Main'
import Settings from './views/Settings'
import RegionPicker from './views/RegionPicker'

function currentRoute(): string {
  return window.location.hash || '#/'
}

function App() {
  const [route, setRoute] = useState(currentRoute)

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
        <Main />
      </div>
      {route === '#/settings' && <Settings />}
    </>
  )
}

export default App
