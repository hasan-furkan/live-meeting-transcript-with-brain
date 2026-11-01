type IconName = 'play' | 'stop' | 'sparkle' | 'camera'

type Props = {
  name: IconName
  size?: number
}

function Icon({ name, size = 18 }: Props) {
  return (
    <svg
      className="icon"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.75}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      {name === 'play' && <path d="M8 5v14l11-7z" />}
      {name === 'stop' && <rect x="6" y="6" width="12" height="12" rx="1" />}
      {name === 'sparkle' && (
        <path d="M12 3l2.2 5.8L20 11l-5.8 2.2L12 19l-2.2-5.8L4 11l5.8-2.2z" />
      )}
      {name === 'camera' && (
        <>
          <path d="M4 8h3l1.5-2h7L17 8h3v11H4z" />
          <circle cx="12" cy="13" r="3.5" />
        </>
      )}
    </svg>
  )
}

export default Icon
