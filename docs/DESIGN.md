# Design contract

Method and rule ids: `docs/DESIGN-METHOD.md`. Workers read this file and the ids named in their task, not the whole method.

## Identity (I-01..I-05)

For people following a live meeting or lecture on one screen, Live Meeting Assistant behaves like a quiet co-pilot's notebook made of dark frosted glass: the next answer is readable at a glance, and what the app hears is always honestly shown.

- **Glass, dark, calm:** floating translucent panels with background blur over a still, dark ambient backdrop; capsule controls; thin light edges; soft depth. The look of a modern native desktop app, not a web dashboard.
- **Readable at a glance:** the suggested answer is the largest reading text on the screen (16/24, strongest text colour), and the answers region takes twice the width of the transcript.
- **Honestly shown:** every state has a word beside its colour. "Recording" appears only while a session runs; no moving meter or animation exists without real data.
- **Exclusions:** gradients only in the ambient backdrop, never on panels, buttons or text; no glow on content; the backdrop never moves; no decorative charts or fake metrics; no marketing hero inside the work area; no third-party logos or brand assets.
- **Tasks:** primary: read suggested answers. Supporting: follow the transcript; start and stop a session; ask now. Occasional: change settings; choose a screen region.

## Platform

Tauri desktop window: default 1280 x 820, minimum 820 x 600. There is no phone layout. Reference sizes are 1440 x 1000, 1280 x 820 and 820 x 600. Mouse-first density decision (C-02): buttons, fields and segments are 40 high; text inputs keep 16/24.

The glass is inside the app window: it blurs the app's own ambient backdrop, not the desktop behind the window. The window itself stays opaque, so text contrast does not depend on the user's wallpaper.

## Tokens

Defined once in `:root` of `ui/src/styles.css`. A new token needs manager review. Colour values appear nowhere else in the CSS.

| Token | Value | Use |
|---|---|---|
| `--canvas` | #06070C | window base colour |
| `--ambient-1` | rgba(62,92,255,.62) | backdrop blob, top left |
| `--ambient-2` | rgba(150,66,225,.52) | backdrop blob, top right |
| `--ambient-3` | rgba(16,160,170,.38) | backdrop blob, bottom |
| `--glass` | rgba(18,20,28,.52) | panels: session, answers, transcript |
| `--glass-bar` | rgba(18,20,28,.58) | top bar, save bar, modal, saving popup |
| `--glass-solid` | #171A24 | fallback for `--glass` and `--glass-bar` |
| `--blur` | saturate(170%) blur(40px) | backdrop filter of all glass |
| `--scrim` | rgba(0,0,0,.45) | backdrop behind the course picker and screenshot overlay |
| `--raised` | rgba(255,255,255,.06) | cards inside panels, secondary buttons, segment track |
| `--hover` | rgba(255,255,255,.12) | hover on raised items, meter track, selected tab and segment |
| `--input` | rgba(0,0,0,.25) | text inputs, selects, textareas |
| `--edge` | rgba(255,255,255,.16) | 1 px border of panels and cards; separators |
| `--edge-top` | rgba(255,255,255,.22) | inset top highlight on glass: `inset 0 1px 0` |
| `--control-border` | rgba(255,255,255,.42) | boundary of inputs, checkboxes, secondary buttons |
| `--text` | #F5F5F7 | body text |
| `--text-strong` | #FFFFFF | titles, suggested answer |
| `--text-secondary` | #BCC1CC | descriptions, labels, metadata, placeholder |
| `--primary` | #F5F5F7 | filled primary button (white capsule) |
| `--primary-hover` | #FFFFFF | primary hover |
| `--primary-text` | #0A0B10 | text on `--primary` |
| `--accent` | #8DBBFF | meter fill, links, switch on, focus ring |
| `--selected` | rgba(110,168,255,.20) | newest-card tint |
| `--selected-text` | #D6E4FF | text on `--selected` |
| `--success` / `--success-bg` | #5EE38B / rgba(94,227,139,.14) | ok chips |
| `--warning` / `--warning-bg` | #FFD60A / rgba(255,214,10,.14) | warning chips and text |
| `--error` / `--error-bg` | #FFA8A1 / rgba(255,168,161,.14) | error text and chips |
| `--recording` | #FF6B62 | recording dot |
| `--danger` / `--danger-hover` / `--danger-text` | #D92D3A / #BE2430 / #FFFFFF | Stop button |

Measured contrast at the theoretical worst backdrop point (all three blobs at full strength on one point, saturated by the blur filter): text on panel 10.47, card 8.86, selected 7.50; secondary on panel 6.32, card 5.35, selected 4.52, hover 4.49; accent on panel 5.80, card 4.91; error on selected 4.42; control border on panel 3.30, input 3.69, card 3.06; primary text on primary 18.05; white on danger 4.79. Hover and selected values just under 4.5 occur only at that point, which the backdrop geometry does not reach (blobs barely overlap). Text placed directly on the backdrop (Settings header, status bar) is measured on one blob at full strength: secondary 5.15 (blue), 6.10 (purple), 6.24 (teal); warning 6.58, error 5.03 (blue).

- **Type:** `-apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif`; mono `"SF Mono", "Cascadia Code", ui-monospace, Consolas, monospace` for times, counts, paths, hotkey only. Scale (size/line height): metadata 12/16, dense body 14/20, reading and input 16/24, card title 20/28, page title 24/32. Weights 400, 500, 600. Titles letter-spacing -0.01 em. Nothing under 12 px. Uppercase only for labels under 24 characters, tracking 0.08 em.
- **Spacing:** only 4, 8, 12, 16, 24, 32, 48, 64.
- **Radius:** buttons, tabs, segments and chips-with-state full capsule; inputs 12; cards inside panels 16; panels, top bar, save bar and modal 24.
- **Elevation:** every glass surface: 1 px `--edge`, `inset 0 1px 0 var(--edge-top)`, `0 8px 32px rgba(0,0,0,.28)`. Modal: `0 24px 64px rgba(0,0,0,.5)`.
- **Glass rule:** `backdrop-filter` and `-webkit-backdrop-filter` (macOS WebKit needs the prefix), both `var(--blur)`, only on the glass surfaces named above. Cards, buttons and inputs are never blurred. No animated property on a blurred surface. Under `@supports not ((backdrop-filter: blur(1px)) or (-webkit-backdrop-filter: blur(1px)))` and `@media (prefers-reduced-transparency: reduce)`, glass uses `--glass-solid` with no filter.
- **Backdrop:** a `position: fixed; inset: 0; z-index: -1` layer in `.app` with three distinct soft colour shapes (`radial-gradient`, sized in vw and vh, solid to 30% and fading out by 88%) over `--canvas`. The shapes must be defined enough that their edges are visible in the gaps and frosted inside the panels; that difference is what makes the glass read as glass. Static. The region picker window does not get it.
- **Motion:** feedback 120 ms, transitions 180 ms ease. Loops only for real work (spinner, recording dot, pending card); all removed under `prefers-reduced-motion`.
- **Focus:** 2 px `--accent` outline, 2 px offset, everywhere.
- **Layers (F-02):** sticky 10, popover 30, toast/saving popup 40, modal backdrop 50, modal 60.

## Live screen, in reading order

The window never scrolls as a page at content width 960 and up: panels fill the height and scroll inside. Panels float with 16 gaps and 16 outer margin; nothing touches the window edge.

1. **Top bar** (floating glass, 56 high, radius 24): logo mark and product name; tabs Live, Settings in the centre as a capsule segmented control (selected tab `--hover` with `--text-strong`); status pill at the far end ("Idle" or "Recording · course", dot pulses only while recording).
2. **Session band** (glass panel, padding 16 vertical, 24 horizontal). At content width 960 and up it is **one row**: left: label "Session" (metadata), H1 with the course or "No session running" (24/32), one sentence: idle "Start a session to transcribe the meeting and get suggested answers."; running "Transcribing system audio on this computer." Middle: the metrics, label above value, all three top-aligned so the labels share one baseline: Speech engine, Audio level (meter 160 wide, 6 high, capsule, with dB), Analyses. Far end: the primary action **Start** (white capsule), or **Stop** (`--danger` capsule). Below 960 it wraps: text and action on the first row, metrics on the second; nothing overflows.
3. **Answers** (dominant, 8 columns, glass panel): head "Questions & suggested answers" with **Ask now** (filled primary only while running; secondary and disabled otherwise) and **Capture & ask** (secondary). Cards newest first, scroll inside.
4. **Transcript** (support, 4 columns, at least 320 wide, glass panel): head "Live transcript"; time in mono above or beside each line; partial line italic secondary, as the last line of the list, flowing right after the completed lines. Both panel heads have the same height (72), so their separators line up.
5. **Status bar** (no glass, transparent, text 12/16 or larger): warnings, errors, notes path, Retry summary. Keeps a fixed minimum height of 32 even when empty, so the layout does not shift; it pulls up 16 into the page gap so the panels end 32 above the window bottom.

Split per L-05: side by side only at content width 960 and up (window width about 1008). Below that, answers first and transcript under them, page scrolls, transcript 240 high, answers at least 320 high.

**Card** (`--raised`, 1 px `--edge`, radius 16, no blur): chip (reason) and time in mono on the first line; question 14/20 `--text-secondary`; label "Suggested answer" metadata; answer 16/24 `--text-strong`; explanation 14/20 secondary; chips: confidence, screenshot state (`--success` / `--warning` with a camera icon, no emoji); thumbnail 160 wide max, radius 12. Newest card (first child) has a `--selected` background, no glow. Every text in a card and in the transcript breaks long unbroken strings (`overflow-wrap: anywhere`) and never leaves its card.

**Empty states (method section 3.3), keeping region height:**

- Answers: title "No answers yet", sentence idle "Questions you are asked will appear here with a suggested answer."; running "Listening for questions. You can also press Ask now."
- Transcript: title "Nothing transcribed yet", sentence idle "The transcript appears here when a session is running."; running "Waiting for speech."

Empty states are centred in their panel, text centred.

**Icons:** one small set in `ui/src/components/Icon.tsx`, inline SVG, stroke 1.75, 18 px in buttons, 16 px in chips, `aria-hidden`: play, stop, sparkle (Ask now), camera. No icon library.

## Settings screen

Header: label "Preferences", H1 "Settings" (24/32), existing sentence. The content column is at most 904 wide and centred in the window. Each section is its own glass panel (radius 24, padding 24), 16 apart: at content width 960 and up a two-column row: left 280 wide title (16/24, weight 600) and description; right the fields, max 560. Below that, stacked. Labels above fields, gap 8; field groups 16. Segment: capsule track `--raised`, 40 high, at least 64 wide per item, selected `--hover` with `--text-strong`. Checkboxes are switches (capsule 40 x 24, on `--accent`) with a visible `--control-border` boundary when off. Save bar: floating glass capsule stuck to the bottom with 16 spare, same width as the content column, message left, Save right, layer 10. It always stays at the bottom of the window while the page scrolls and never covers the focused control (`scroll-padding-bottom`). Save is filled only when dirty.

Sticky note: `padding-bottom` on the scrolling element moves a sticky bar up by that amount. Do not put bottom padding on the scroller; the save bar is the last item in the flow, so nothing needs clearing.

## Other surfaces

- Course picker modal: 440 wide, glass bar, radius 24, title 20/28, course buttons full width, OK filled, focus stays visible, backdrop `rgba(0,0,0,.45)`.
- Saving popup stays centred and non-modal, glass bar.
- Region picker: tokens only, no layout change.
- `backdrop-filter` makes an element the containing block of `position: fixed` children: modals, the saving popup and any other fixed element must not be inside a blurred element.

## Dropdowns

No native `<select>`: its popup cannot follow the design. `ui/src/components/Select.tsx` is the only dropdown. Trigger looks like a text input with a chevron. The list is opaque `--glass-solid` with no blur, radius 16, padding 4, rows radius 12, active row `--hover`, selected row `--selected` with a check; it opens in a portal on `document.body` at layer 30, because glass panels are stacking contexts and containing blocks for fixed children.
