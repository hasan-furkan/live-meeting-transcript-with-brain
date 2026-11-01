import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
} from 'react'
import { createPortal } from 'react-dom'

export type SelectOption = {
  value: string
  label: string
  primary?: string
  secondary?: string
}

type Props = {
  id: string
  value: string
  options: SelectOption[]
  onChange: (value: string) => void
  onOpen?: () => void
}

const MAX_LIST_HEIGHT = 280
const EDGE = 8
const GAP = 4
const TYPEAHEAD_RESET = 500

function Select({ id, value, options, onChange, onOpen }: Props) {
  const [open, setOpen] = useState(false)
  const [storedActive, setActive] = useState(0)
  const triggerRef = useRef<HTMLButtonElement>(null)
  const listRef = useRef<HTMLDivElement>(null)
  const typeahead = useRef<{ text: string; timer: number | undefined }>({
    text: '',
    timer: undefined,
  })
  const listId = `${id}-list`
  // The window list refreshes while the list is open, so it can shrink under the active row.
  const active = Math.min(storedActive, Math.max(0, options.length - 1))

  const selectedIndex = options.findIndex((option) => option.value === value)
  const selectedOption = selectedIndex >= 0 ? options[selectedIndex] : undefined
  const shownOption = selectedOption ?? options.find((option) => option.value === '')
  const shownLabel = shownOption?.label ?? ''

  const openList = useCallback(() => {
    if (open) return
    onOpen?.()
    const selected = options.findIndex((option) => option.value === value)
    const empty = options.findIndex((option) => option.value === '')
    setActive(selected >= 0 ? selected : empty >= 0 ? empty : 0)
    setOpen(true)
  }, [onOpen, open, options, value])

  const closeList = useCallback((focusTrigger: boolean) => {
    setOpen(false)
    if (focusTrigger) triggerRef.current?.focus()
  }, [])

  const choose = useCallback(
    (index: number) => {
      const option = options[index]
      if (option) onChange(option.value)
      setOpen(false)
    },
    [onChange, options],
  )

  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (!open) {
      if (
        event.key === 'ArrowDown' ||
        event.key === 'ArrowUp' ||
        event.key === 'Enter' ||
        event.key === ' '
      ) {
        event.preventDefault()
        openList()
      }
      return
    }

    switch (event.key) {
      case 'Escape':
        event.preventDefault()
        closeList(true)
        return
      case 'Tab':
        setOpen(false)
        return
      case 'ArrowDown':
        event.preventDefault()
        setActive((current) => Math.min(current + 1, options.length - 1))
        return
      case 'ArrowUp':
        event.preventDefault()
        setActive((current) => Math.max(current - 1, 0))
        return
      case 'Home':
        event.preventDefault()
        setActive(0)
        return
      case 'End':
        event.preventDefault()
        setActive(Math.max(0, options.length - 1))
        return
      case 'Enter':
      case ' ':
        event.preventDefault()
        choose(active)
        return
      default:
        break
    }

    if (
      event.key.length === 1 &&
      !event.ctrlKey &&
      !event.metaKey &&
      !event.altKey &&
      options.length > 0
    ) {
      const state = typeahead.current
      state.text += event.key.toLowerCase()
      window.clearTimeout(state.timer)
      state.timer = window.setTimeout(() => {
        state.text = ''
      }, TYPEAHEAD_RESET)
      for (let step = 1; step <= options.length; step += 1) {
        const index = (active + step) % options.length
        const option = options[index]
        if (option && option.label.toLowerCase().startsWith(state.text)) {
          setActive(index)
          break
        }
      }
    }
  }

  // The list is fixed in a portal: glass panels are stacking contexts and
  // containing blocks for fixed children, so it cannot live inside the panel.
  useLayoutEffect(() => {
    if (!open) return
    const trigger = triggerRef.current
    const list = listRef.current
    if (!trigger || !list) return
    const rect = trigger.getBoundingClientRect()
    const width = rect.width
    list.style.width = `${width}px`
    const contentHeight = Math.min(list.scrollHeight, MAX_LIST_HEIGHT)
    const spaceBelow = window.innerHeight - rect.bottom - GAP - EDGE
    const spaceAbove = rect.top - GAP - EDGE
    const placeAbove = contentHeight > spaceBelow && spaceAbove > spaceBelow
    const height = Math.max(0, Math.min(contentHeight, placeAbove ? spaceAbove : spaceBelow))
    const left = Math.max(EDGE, Math.min(rect.left, window.innerWidth - width - EDGE))
    const top = Math.max(EDGE, placeAbove ? rect.top - GAP - height : rect.bottom + GAP)
    list.style.left = `${left}px`
    list.style.top = `${top}px`
    list.style.maxHeight = `${height}px`
  }, [open, options.length])

  // Keep the active row visible while the keyboard moves through the list.
  useEffect(() => {
    if (!open) return
    const row = document.getElementById(`${id}-option-${active}`)
    row?.scrollIntoView?.({ block: 'nearest' })
  }, [active, id, open])

  // Close on a press outside the trigger and the list, on resize, and on any
  // ancestor scroll (the list's own scroll must not close it).
  useEffect(() => {
    if (!open) return
    const onMouseDown = (event: MouseEvent) => {
      const target = event.target
      if (!(target instanceof Node)) return
      if (triggerRef.current?.contains(target)) return
      if (listRef.current?.contains(target)) return
      setOpen(false)
    }
    const onScroll = (event: Event) => {
      const target = event.target
      if (target instanceof Node && listRef.current?.contains(target)) return
      setOpen(false)
    }
    const onResize = () => setOpen(false)
    document.addEventListener('mousedown', onMouseDown)
    window.addEventListener('scroll', onScroll, true)
    window.addEventListener('resize', onResize)
    return () => {
      document.removeEventListener('mousedown', onMouseDown)
      window.removeEventListener('scroll', onScroll, true)
      window.removeEventListener('resize', onResize)
    }
  }, [open])

  useEffect(() => {
    const state = typeahead.current
    return () => window.clearTimeout(state.timer)
  }, [])

  const list =
    open &&
    createPortal(
      <div
        ref={listRef}
        id={listId}
        className="select-list"
        role="listbox"
        onMouseDown={(event) => event.preventDefault()}
      >
        {options.map((option, index) => {
          const selected = index === selectedIndex
          const className = `select-option${index === active ? ' active' : ''}${
            selected ? ' selected' : ''
          }`
          return (
            <div
              key={`${option.value}-${index}`}
              id={`${id}-option-${index}`}
              className={className}
              role="option"
              aria-selected={selected}
              aria-label={option.label}
              onMouseEnter={() => setActive(index)}
              onClick={() => choose(index)}
            >
              {option.primary !== undefined ? (
                <span className="select-option-text">
                  <span className="select-option-primary">{option.primary}</span>
                  {option.secondary && (
                    <span className="select-option-secondary">{option.secondary}</span>
                  )}
                </span>
              ) : (
                <span className="select-option-text">{option.label}</span>
              )}
              {selected && (
                <svg
                  className="select-check"
                  width="16"
                  height="16"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth={1.75}
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  aria-hidden="true"
                  focusable="false"
                >
                  <path d="M5 12.5 10 17.5 19 6.5" />
                </svg>
              )}
            </div>
          )
        })}
      </div>,
      document.body,
    )

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        id={id}
        className="select-trigger"
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listId}
        aria-activedescendant={
          open && options.length > 0 ? `${id}-option-${active}` : undefined
        }
        onClick={() => {
          if (open) setOpen(false)
          else openList()
        }}
        onKeyDown={onKeyDown}
      >
        <span className="select-value">{shownLabel}</span>
        <svg
          className="select-chevron"
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth={1.75}
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
          focusable="false"
        >
          <path d="m6 9 6 6 6-6" />
        </svg>
      </button>
      {list}
    </>
  )
}

export default Select
