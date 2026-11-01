import { fireEvent, render, screen } from '@testing-library/react'
import { expect, test, vi } from 'vitest'

import Select, { type SelectOption } from './Select'

const OPTIONS: SelectOption[] = [
  { value: '', label: 'Choose a fruit…' },
  { value: 'apple', label: 'Apple' },
  { value: 'banana', label: 'Banana' },
  { value: 'cherry', label: 'Cherry' },
]

function setup(value = '', options: SelectOption[] = OPTIONS) {
  const onChange = vi.fn()
  const onOpen = vi.fn()
  const result = render(
    <Select id="pick" value={value} options={options} onChange={onChange} onOpen={onOpen} />,
  )
  return { ...result, onChange, onOpen, trigger: screen.getByRole('combobox') }
}

test('closed_shows_the_selected_label_and_no_listbox', () => {
  const { trigger } = setup('banana')

  expect(screen.queryByRole('listbox')).toBeNull()
  expect(trigger).toHaveAttribute('aria-expanded', 'false')
  expect(trigger).toHaveTextContent('Banana')
})

test('click_opens_once_and_marks_the_selected_option', () => {
  const { trigger, onOpen } = setup('banana')

  fireEvent.click(trigger)

  expect(onOpen).toHaveBeenCalledTimes(1)
  expect(trigger).toHaveAttribute('aria-expanded', 'true')
  expect(screen.getByRole('listbox')).toBeInTheDocument()
  expect(screen.getByRole('option', { name: 'Banana' })).toHaveAttribute('aria-selected', 'true')
  expect(screen.getByRole('option', { name: 'Apple' })).toHaveAttribute('aria-selected', 'false')
  expect(screen.getByRole('option', { name: 'Cherry' })).toHaveAttribute('aria-selected', 'false')
})

test('choosing_an_option_reports_its_value_and_closes', () => {
  const { trigger, onChange } = setup()

  fireEvent.click(trigger)
  fireEvent.click(screen.getByRole('option', { name: 'Banana' }))

  expect(onChange).toHaveBeenCalledTimes(1)
  expect(onChange).toHaveBeenCalledWith('banana')
  expect(trigger).toHaveAttribute('aria-expanded', 'false')
  expect(screen.queryByRole('listbox')).toBeNull()
})

test('escape_closes_without_choosing_and_focuses_the_trigger', () => {
  const { trigger, onChange } = setup()

  fireEvent.click(trigger)
  fireEvent.keyDown(trigger, { key: 'Escape' })

  expect(screen.queryByRole('listbox')).toBeNull()
  expect(onChange).not.toHaveBeenCalled()
  expect(document.activeElement).toBe(trigger)
})

test('arrow_keys_move_the_active_row_and_enter_chooses_it', () => {
  const { trigger, onChange } = setup()

  fireEvent.click(trigger)
  fireEvent.keyDown(trigger, { key: 'ArrowDown' })
  fireEvent.keyDown(trigger, { key: 'ArrowDown' })
  expect(trigger).toHaveAttribute('aria-activedescendant', 'pick-option-2')
  fireEvent.keyDown(trigger, { key: 'Enter' })

  expect(onChange).toHaveBeenCalledWith('banana')
  expect(screen.queryByRole('listbox')).toBeNull()
})

test('arrow_up_stops_at_the_first_row_and_home_end_jump_to_the_edges', () => {
  const { trigger } = setup()

  fireEvent.click(trigger)
  fireEvent.keyDown(trigger, { key: 'ArrowUp' })
  expect(trigger).toHaveAttribute('aria-activedescendant', 'pick-option-0')
  fireEvent.keyDown(trigger, { key: 'End' })
  expect(trigger).toHaveAttribute('aria-activedescendant', 'pick-option-3')
  fireEvent.keyDown(trigger, { key: 'Home' })
  expect(trigger).toHaveAttribute('aria-activedescendant', 'pick-option-0')
})

test('aria_activedescendant_identifies_a_rendered_option', () => {
  const { trigger } = setup()

  fireEvent.click(trigger)
  const activeId = trigger.getAttribute('aria-activedescendant')
  expect(activeId).not.toBeNull()
  const row = activeId === null ? null : document.getElementById(activeId)
  expect(row).not.toBeNull()
  expect(row?.getAttribute('role')).toBe('option')
})

test('tab_closes_without_choosing', () => {
  const { trigger, onChange } = setup()

  fireEvent.click(trigger)
  fireEvent.keyDown(trigger, { key: 'Tab' })

  expect(screen.queryByRole('listbox')).toBeNull()
  expect(onChange).not.toHaveBeenCalled()
})

test('typing_a_letter_activates_the_row_starting_with_it', () => {
  const { trigger, onChange } = setup()

  fireEvent.click(trigger)
  fireEvent.keyDown(trigger, { key: 'b' })
  expect(trigger).toHaveAttribute('aria-activedescendant', 'pick-option-2')
  fireEvent.keyDown(trigger, { key: 'Enter' })

  expect(onChange).toHaveBeenCalledWith('banana')
})

test('mouse_down_outside_closes_but_inside_the_list_keeps_it_open', () => {
  const { trigger } = setup()

  fireEvent.click(trigger)
  fireEvent.mouseDown(screen.getByRole('listbox'))
  expect(screen.getByRole('listbox')).toBeInTheDocument()

  fireEvent.mouseDown(document.body)
  expect(screen.queryByRole('listbox')).toBeNull()
})

test('window_resize_closes_the_list', () => {
  const { trigger } = setup()

  fireEvent.click(trigger)
  fireEvent(window, new Event('resize'))

  expect(screen.queryByRole('listbox')).toBeNull()
})

test('scrolling_an_ancestor_closes_the_list_but_scrolling_inside_does_not', () => {
  const onChange = vi.fn()
  render(
    <div data-testid="scroller">
      <Select id="pick" value="" options={OPTIONS} onChange={onChange} />
    </div>,
  )

  fireEvent.click(screen.getByRole('combobox'))
  fireEvent.scroll(screen.getByRole('listbox'))
  expect(screen.getByRole('listbox')).toBeInTheDocument()

  fireEvent.scroll(screen.getByTestId('scroller'))
  expect(screen.queryByRole('listbox')).toBeNull()
})

test('unmounting_while_open_removes_the_list_from_the_document', () => {
  const { trigger, unmount } = setup()

  fireEvent.click(trigger)
  expect(screen.getByRole('listbox')).toBeInTheDocument()

  unmount()
  expect(screen.queryByRole('listbox')).toBeNull()
})

test('a_label_that_looks_like_html_is_rendered_as_text', () => {
  const hostile = '<img src=x onerror=alert(1)>'
  const options: SelectOption[] = [
    { value: '', label: 'Choose…' },
    { value: 'x', label: hostile },
  ]
  const { trigger } = setup('x', options)

  expect(trigger).toHaveTextContent(hostile)
  expect(document.querySelector('img')).toBeNull()

  fireEvent.click(trigger)
  expect(screen.getByRole('option', { name: hostile })).toHaveTextContent(hostile)
  expect(document.querySelector('img')).toBeNull()
})

test('a_very_long_label_renders', () => {
  const long = 'x'.repeat(5000)
  const options: SelectOption[] = [
    { value: '', label: 'Choose…' },
    { value: 'long', label: long },
  ]
  const { trigger } = setup('long', options)

  expect(trigger.textContent).toBe(long)

  fireEvent.click(trigger)
  expect(screen.getByRole('option', { name: long }).textContent).toBe(long)
})

test('an_unknown_value_falls_back_to_the_empty_option_label', () => {
  const { trigger } = setup('missing')

  expect(trigger).toHaveTextContent('Choose a fruit…')
  expect(trigger).not.toHaveTextContent('Banana')
})

test('a list that shrinks while open keeps the active row on a row that exists', () => {
  const longList = [
    { value: 'a', label: 'Alpha' },
    { value: 'b', label: 'Beta' },
    { value: 'c', label: 'Gamma' },
  ]
  const onChange = vi.fn()
  const { rerender } = render(
    <Select id="shrink" value="c" options={longList} onChange={onChange} />,
  )
  fireEvent.click(screen.getByRole('combobox'))
  rerender(<Select id="shrink" value="c" options={longList.slice(0, 2)} onChange={onChange} />)
  const trigger = screen.getByRole('combobox')
  const activeId = trigger.getAttribute('aria-activedescendant')
  expect(activeId).not.toBeNull()
  expect(document.getElementById(activeId ?? '')).toHaveTextContent('Beta')
  fireEvent.keyDown(trigger, { key: 'Enter' })
  expect(onChange).toHaveBeenCalledWith('b')
})
