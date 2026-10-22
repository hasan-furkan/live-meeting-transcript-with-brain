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

  if (route === '#/settings') return <Settings />
  if (route === '#/region') return <RegionPicker />
  return <Main />
}

export default App
