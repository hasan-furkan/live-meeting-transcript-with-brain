export type SessionStatus = { running: boolean; course: string }

export const IDLE: SessionStatus = { running: false, course: '' }

type Props = {
  active: 'live' | 'settings'
  status?: SessionStatus
}

function Header({ active, status = IDLE }: Props) {
  return (
    <header className="topbar">
      <div className="brand">
        <span className="logo-mark" aria-hidden="true" />
        <span className="brand-name">Live Meeting Assistant</span>
      </div>
      <nav className="tabs" aria-label="Primary">
        <a
          href="#/"
          className={active === 'live' ? 'tab active' : 'tab'}
          aria-current={active === 'live' ? 'page' : undefined}
        >
          Live
        </a>
        <a
          href="#/settings"
          className={active === 'settings' ? 'tab active' : 'tab'}
          aria-current={active === 'settings' ? 'page' : undefined}
        >
          Settings
        </a>
      </nav>
      <span className={status.running ? 'status-pill live' : 'status-pill'}>
        <span className="dot" aria-hidden="true" />
        <span className="status-text">
          {status.running ? `Recording · ${status.course}` : 'Idle'}
        </span>
      </span>
    </header>
  )
}

export default Header
