# Design Method: From Product Identity to Verifiable Screens

Version 1.0 · 2026-10-30 · Language: English

This is a design decision process for a managing AI and implementation workers, including DeepSeek workers. It contains rules, measurements, decision tables and acceptance criteria; it is not a code template.

## Basis and interpretation

The method is synthesized from the following inspected local sources:

- **FotoSosyal:** (local sample project) and its project README: ivory paper, archive green, burnt-orange actions, editorial serif headings, photographic content and a contact-sheet metaphor.
- **Focus:** (local sample project) and its project README: dark blue surfaces, mint interaction cues, a prominent voice interaction stage, visible listening/processing/playback state and supporting conversation panels.
- **Pulse Monitor:** (local sample project), its components and contract: dark telemetry surfaces, cyan actions, separate chart series, metric cards, service summaries, logs and secondary event injection.

The identity sentences below reconstruct the visual logic of these products; they are not quotations from historical briefs. Measurements below standardize that logic for future work. Existing tiny labels, faint text or accidental layout choices are not precedents to copy.

**Rule precedence:** accessibility and task completion → truthful information → content hierarchy → identity → decoration. **Reason:** a recognizable theme cannot compensate for an unreadable or unusable product.

**Measurement convention:** dimensions are CSS pixels at the default 16 px root size. Express typography and text-dependent spacing in scalable units during implementation; never lock the root size to defeat browser preferences. **Reason:** the specification needs reproducible measurements while remaining usable with larger text.

**Status of numbers:** values marked **house rule** are deliberate project defaults, not universal research findings or WCAG requirements. Accessibility floors are identified and linked separately. **Reason:** another AI must know what may be deliberately adapted and what must not be weakened.

## 1. Write an identity sentence, then derive tokens

### 1.1 Identity procedure

| Step | Required decision | Reason |
| --- | --- | --- |
| I-01 | Write one sentence of **20–35 words** containing: audience, primary task, use context, two behavioral qualities and one concrete visual metaphor. | These inputs constrain the design more effectively than a list of fashionable adjectives. |
| I-02 | Use this sentence pattern: “For [audience] doing [task] in [context], [product] behaves like [metaphor]: [quality with a behavioral consequence] and [second quality with a consequence].” | The sentence links appearance to work the user actually performs. |
| I-03 | Convert each quality into at least **one measurable choice** before choosing colors: e.g. calm → one dominant action and no idle looping animation; precise → aligned numeric columns and persistent status labels. | An adjective without a consequence does not guide a worker. |
| I-04 | Name **three visual exclusions**, such as “no marketing hero inside the work area,” “no neon on every surface,” or “no decorative handwriting in controls.” | Exclusions prevent unrelated local choices from diluting the identity. |
| I-05 | State the primary task in a verb–object phrase and rank all other tasks as supporting or occasional. | Task priority determines layout and action emphasis. |

### 1.2 Derivation sequence

Derive tokens in the following order; do not start by selecting a component library theme.

| Token family | Derivation rule | Reason |
| --- | --- | --- |
| Color roles | Define canvas, surface, raised surface, primary text, secondary text, decorative divider, control boundary, action, action text, selected state, focus, success, warning and error. Give chart series their own roles. | Semantic roles keep a worker from reusing one attractive color for incompatible meanings. |
| Palette | Choose **one action hue**, **one neutral family** and at most **one secondary brand hue**. Reserve semantic colors for their meanings; chart colors do not count as actions. | A limited palette creates a stable visual hierarchy without hiding status. |
| Color validation | Measure every text/surface pair and every essential control or graph boundary. Composite transparent layers over the actual background before measuring. | Hex values and opacity percentages alone cannot establish readability. |
| Typography | Choose one UI family; add one display family only when the metaphor requires it, and one monospace family only for numeric/technical data. Maximum **three families**, including fallbacks by role. | Type must distinguish reading, navigation and data rather than decorate every component. |
| Spacing | Start from a **4 px base**. Use only **4, 8, 12, 16, 24, 32, 48, 64** for layout spacing. A **2 px exception** is allowed only for optical icon alignment. | A finite scale makes related elements visibly related. |
| Radius | Select one control radius and one panel radius from the identity examples. A nested panel radius must not exceed its parent radius. Use a full pill only for statuses or genuinely segmented controls. | Shape becomes a coherent family rather than a set of unrelated containers. |
| Shadow | Define at most **three elevation levels**: none, panel, floating overlay. Keep accent glow off ordinary content cards. | Elevation should explain layering, not make all content compete. |
| Motion | Define feedback, transition and optional meaningful-state motion separately. Use the timing table below; the reduced-motion variant removes decorative movement. | Consistent motion communicates a change without becoming background noise. |
| Token ownership | Record value, semantic role, permitted use, prohibited use and rationale for each token. Workers consume the shared record; a new token requires a stated missing role and manager review. | Shared decisions prevent each module from inventing its own design system. |

### 1.3 Three worked derivations

These are token specifications, not code. A future product may adopt one family after writing its own identity sentence.

**A. FotoSosyal — an editorial archive**

“For photographers browsing and sharing everyday work at an unhurried pace, FotoSosyal behaves like an editorial contact sheet: images lead, and publishing feels deliberate rather than promotional.”

| Family | Derived choice | Reason |
| --- | --- | --- |
| Color | Canvas **#F8F2E7**, paper **#FFFDF8**, text **#1B211D**, secondary text **#666C64**, archive green **#1A4032** for navigation, action **#B94A1B** with **#FFFDF8** text; hover **#8F3512**. | Warm paper supports photography; a separate burnt-orange action prevents green navigation from competing with publishing. |
| Type | Georgia or the available Iowan/Palatino serif family for headings; Segoe UI/system sans for controls and reading; monospace only for technical administration. H1 **36/40**, card title **20/28**, body **16/24**. | Editorial headings establish identity while ordinary reading and controls remain familiar. |
| Spacing | Feed gap **24**, desktop panel padding **24**, photo-to-caption gap **16**, label-to-field gap **8**, section separation **48**. | Generous separation gives images priority without adding empty hero space. |
| Shape | Controls **8** radius, panels **16**, image corners **8**, avatar/status pill only when semantically appropriate. | Moderate rounding supports a tactile paper metaphor without turning every object into a bubble. |
| Elevation | Panel: vertical offset **2**, blur **8**, spread **0**, black opacity **6%**. Overlay: offset **12**, blur **32**, spread **0**, black opacity **18%**. | Surfaces feel layered while the image remains the highest-detail object. |
| Motion | Feedback **120 ms**, panel transition **180 ms**, optional image hover scale at most **1.015**, disabled under reduced motion. | Restrained feedback preserves the unhurried identity. |
| Structure consequence | The feed is the dominant region; a short posting composer belongs at the feed start or behind a clearly named publish action. | Publishing acts on the feed and should retain that context. |

**B. Focus — a calm conversation instrument**

“For English learners practicing aloud alone, Focus behaves like a calm conversation instrument: the next speaking action is unmistakable, and listening, processing and playback always remain visibly distinct.”

| Family | Derived choice | Reason |
| --- | --- | --- |
| Color | Canvas **#060B11**, opaque panel reference **#101C29**, text **#E9F2F4**, secondary text **#8DA3B1**, action **#6EE7C7** with **#060B11** text; hover **#9CF1D9**. Cyan is supporting information; rose indicates error/stop consequences. | Mint identifies participation while state remains readable without requiring a bright background. |
| Type | One Segoe UI/system sans family; monospace only for timers and device/diagnostic data. H1 **32/40**, primary state **24/32**, conversation body **16/24**. | A stable voice interaction should not look like a dense engineering console. |
| Spacing | Main-stage padding **32**, control group gap **12**, conversation padding **24**, panel gap **24**. | The speaking action gets an unmistakable center; supporting content remains nearby. |
| Shape | Controls **12** radius, panels **16**, a circular voice indicator only for the interaction state. | Softer controls support reassurance without imposing circular styling on data or forms. |
| Elevation | Ordinary panels: none beyond a divider. Overlay: offset **16**, blur **40**, spread **0**, black opacity **28%**. Ambient color tint at most **8%** after compositing. | Distinction comes from structure and state, not stacked luminous surfaces. |
| Motion | Feedback **120 ms**, state transition **200 ms**. A state pulse, if used, lasts **1600 ms** per cycle and runs only during actual listening; a static label remains present. No fabricated audio waveform. | Motion may acknowledge capture but must not imply audio or a state that does not exist. |
| Structure consequence | Start/Talk/Stop controls occupy the primary stage before the transcript; window/settings controls are secondary. | The user first needs to know whether to speak and how to stop. |

**C. Pulse Monitor — an operations instrument**

“For operators scanning changing service health during a local investigation, Pulse Monitor behaves like an operations instrument: anomalies surface immediately, and dense evidence stays aligned, stable and inspectable.”

| Family | Derived choice | Reason |
| --- | --- | --- |
| Color | Canvas **#05080D**, panel **#0A1019**, text **#E8EEF6**, secondary text **#93A6BA**, action **#22D3EE** with **#041419** text; hover **#67E8F9**. CPU **#3987E5**, RAM **#D95926**; reserve error styling for actual error states. | Actions, series and incidents must remain distinguishable while sharing a dark neutral foundation. |
| Type | Segoe UI/system sans UI; Cascadia/Consolas monospace for timestamps and metrics. H1 **32/40**, metric **32/40**, table/body **14/20**, essential metadata **12/16**. | Alignment and numeric stability matter more than expressive typography in investigation. |
| Spacing | Card gap **16**, panel padding **24**, metric internal gaps **8/12**, major section gap **24**, table cell horizontal padding **16**. | A tighter but explicit rhythm increases evidence density without shrinking labels. |
| Shape | Controls **8** radius, panels **16**, severity chip **4** radius. | Squarer status marks read as classified data rather than conversational bubbles. |
| Elevation | Ordinary data panels: no outer shadow; **1 px** decorative divider. Floating tools: offset **8**, blur **24**, spread **0**, black opacity **24%**. | Operational hierarchy comes from placement, typography and status, not universal elevation. |
| Motion | Feedback **100 ms**, disclosure transition **160 ms**; chart updates retain a fixed numeric scale. No looping “live” pulse is required; show status and last-update time. | Moving data is already a source of motion; extra animation makes scanning harder. |
| Structure consequence | Metrics and evidence precede the manual injection form; the form occupies a secondary rail when the width budget permits it. | Log injection supports investigation but is not the screen's principal task. |

**Measured examples:** FotoSosyal secondary text/paper **5.30:1**, action text/action **5.09:1**; Focus secondary text/panel **6.56:1**, action text/action **13.08:1**; Pulse secondary text/panel **7.63:1**, action text/action **10.38:1**, CPU/panel **5.24:1**, RAM/panel **4.91:1**. Ratios are rounded for reporting; acceptance uses unrounded values. Recalculate after changing opacity, backgrounds or states. **Reason:** copied token values do not guarantee contrast in a different composition.

## 2. Build the page skeleton from task priority

### 2.1 Header, grid and card hierarchy

| ID | Rule and measurement | Reason |
| --- | --- | --- |
| L-01 | Use this page-header order: optional location/breadcrumb → one H1 naming the task or object → one explanatory sentence → status and action group. H1 is **28/36 mobile**, **32/40 desktop**; an editorial identity may use **36/40 desktop**. Description maximum **65 characters per line**, with at most **two lines** in the reference desktop layout. | Users identify the screen before interpreting its controls. |
| L-02 | Keep header text start-aligned; align its action group to the same content edge. Limit the group to **one primary** and **two secondary** actions; move additional commands into a named menu. | Header controls should not become an alternative toolbar competing with the content. |
| L-03 | Set content maximum width to **1440**, centered; outer gutters **16 below 640**, **24 from 640–1023**, **32 at 1024 and above**. Reading-only text remains **45–75 characters per line**, independent of dashboard width. | Large data regions need space; prose still needs controlled line length. |
| L-04 | Use **12 columns** for wide workspaces, **8** for intermediate layouts and **4** for narrow layouts. Gaps are **24 desktop**, **16 intermediate**, **12 narrow**. Items align to shared column edges, not individually centered positions. | A shared grid makes the screen coherent across independently implemented modules. |
| L-05 | Default split is **8:4** main/support, or approximately **2:1** after the gap. Permit it only when available content width is at least **960**, main width at least **600**, and support width at least **320**. Otherwise stack. | A ratio must not create an unusable form or chart at a particular viewport. |
| L-06 | Persistent navigation is **240–264** wide, chosen once per product. Keep it persistent only at viewport widths of at least **1280** and when L-05's remaining-width budget survives; otherwise use a drawer. | Navigation must not consume the working area needed by the actual task. |
| L-07 | Choose one dominant region before making cards. Give it at least **twice the grid span or visible area** of a supporting card in the reference desktop composition. A row of equal metric summaries is allowed above it. | Dominance must be visible without reading every title. |
| L-08 | Use three content levels: primary workspace, supporting evidence and quiet metadata. Primary: full text contrast and largest allocated area. Supporting: ordinary panel. Metadata: no extra outer card unless it has its own action or lifecycle. | Nested boxes are not a substitute for information hierarchy. |
| L-09 | Standard panel padding is **24 desktop / 16 mobile**; title-to-content gap **16**; section-to-section gap **24 or 32**, selected once. Maximum **two panel nesting levels**. | Repeated spacing explains grouping without excessive borders. |
| L-10 | Put at most **four metric summaries** in a row; change **4 → 2 → 1 columns** as minimum card width **200** is violated. Every metric has label, value, unit and time window/source. | Numeric tiles need comparable meaning, not merely large numbers. |
| L-11 | Reserve media and chart space before data arrives. Photo grid thumbnails use one declared aspect ratio; preserve an original/full-view option when cropping matters. Charts have at least **240** plotting-area height on desktop and **200** on mobile. | Stable geometry prevents layout jumps and makes the primary material inspectable. |

### 2.2 Decide where the action area belongs

Apply these rows in order; use the first matching task condition, then apply the width constraints.

| Condition | Placement rule | Reason |
| --- | --- | --- |
| The action initiates the product's main session, as in Focus | Put the action in the primary central stage, before settings and secondary history. At **390 × 844**, the initial action must be visible without scrolling. | The user should not search for the step that starts the product. |
| The action creates an item in a feed, as in FotoSosyal | Place a short composer at the start of that feed. If it needs more than **four fields** or media editing, use a dedicated compose view or dialog opened from the feed. | Immediate context is useful; a long form should not displace the feed itself. |
| The action is occasional and modifies the evidence being examined, as in Pulse Monitor | Use a right support rail at **320–400** width when L-05 passes. Otherwise put it immediately after the relevant evidence region and provide a named jump action near that region's heading. | The action remains available without outranking the investigation. |
| The action applies only to one row or card | Keep it inside that row/card or its contextual menu; keep destructive commands out of the page's primary action position. | Spatial association prevents acting on the wrong object. |
| The action commits a form | Place submit after the last field, start-aligned in reading order; place cancel beside it as a lower-emphasis action with **8–12** gap. On mobile, make submit full width. | The reading and completion paths should end at the same place. |
| Considering sticky placement | Allow it only when the user needs the action while inspecting content taller than one viewport, and all fixed-element rules in section 4 pass. Never fix a form merely to fill a rail. | Sticky behavior must solve a task problem rather than conceal a weak layout. |

## 3. Component rules and state contracts

### 3.1 Buttons, segmented choices and fields

| ID | Rule and measurement | Reason |
| --- | --- | --- |
| C-01 | One filled primary action per task region; on initial display one region gets the strongest emphasis. Secondary buttons use a boundary/neutral fill; tertiary commands use text or a quiet icon. Destructive actions use semantic error treatment, never the ordinary brand color. | Hierarchy identifies the next step and distinguishes its consequences. |
| C-02 | Default interactive target **44 × 44 minimum**; standard button height **44**, horizontal padding **16**, icon **18–20**, icon/text gap **8**. Dense desktop mode may use **40** height only after an explicit density decision; touch targets remain **44**. | A controlled density mode prevents random shrinking of controls. |
| C-03 | Label actions with a verb and object where the object is not obvious: “Inject event,” “Publish photo,” “Start listening.” Icon-only buttons require an accessible name; tooltips are additional help, not the only label for a primary action. | Users should predict the result before activating the control. |
| C-04 | Specify default, hover, keyboard focus, active, disabled, pending, success and failure behavior for each asynchronous action. Pending keeps button width stable, prevents duplicate submission and preserves its action label with a progress indicator. | State transitions must not move the target or create duplicate work. |
| C-05 | Use a segmented selector for **2–4 mutually exclusive choices**. Every segment is at least **44 high** and **64 wide**; labels do not wrap. Selected state uses text plus shape/boundary, not hue alone. If it cannot fit, use a labeled select or a vertical radio group; do not wrap segments into two rows. | Segmentation is useful only while the alternatives remain directly comparable. |
| C-06 | Treat a segment group as one labeled radio group for choice, or tabs only when it switches associated panels. Support the corresponding keyboard behavior and a visible focus indicator. The entire visible segment activates it. | A visual pill does not define the interaction semantics. |
| C-07 | Forms are single-column by default. Use paired fields only for tightly related short values and only when each field remains at least **240** wide. Labels stay visible above fields; label gap **8**, field-group gap **16**, group separation **24**. | Reading order and error association remain unambiguous. |
| C-08 | Inputs are **44 high**, text **16/24**, horizontal padding **12**, radius from the selected identity. Textareas start at **104** high and grow or resize vertically. Placeholder text is an example, never the sole label. | Fields remain usable on touch devices and after an example disappears. |
| C-09 | Show required/optional conventions once per form; show units, limits and formatting guidance before submission. Put each error below its field with **4–8** gap and text/icon identification; preserve all entered values after a server failure. | Corrective information should appear where the user can act on it. |
| C-10 | Validate after blur or submit rather than announcing errors on untouched fields. On failed submit focus the first invalid field, or an error summary when there are **three or more** errors. Never show success until the operation is confirmed. | Timely feedback corrects mistakes without treating an unfinished form as wrong. |

### 3.2 Tables, chips and live information

| ID | Rule and measurement | Reason |
| --- | --- | --- |
| C-11 | Use tables for cross-row comparison, lists for independent records and cards for media-led browsing. A table's visible columns each need a named user question; remove redundant presentation, not required information. | The representation should match how the information is inspected. |
| C-12 | Table header text **12/16**, body **14/20** or larger; header height at least **40**, single-line row at least **48**; horizontal cell padding **16**, vertical padding **12**. Right-align comparable numbers, use tabular numerals, start-align text and keep units consistent. | Density comes from alignment and content selection rather than tiny text. |
| C-13 | Let log messages wrap; initially clamp only when an adjacent expand control exposes the complete value. Timestamps may use a two-line date/time treatment. Never truncate the only visible severity, record identifier or required row action. | Investigation requires access to evidence, not just a compact preview. |
| C-14 | Default page size **20**, alternatives **50/100** when appropriate. State “1–20 of 237,” preserve filters between pages, reset to page 1 on a filter change and visibly identify sort order. During live updates preserve a user's inspection position or offer pause/new-items handling. | The table must remain navigable while its data changes. |
| C-15 | Place search and filters immediately above the data they affect. Search debounce default **300 ms**; indicate pending results and retain last confirmed data with an updating/stale label instead of flashing an empty table. | Context and continuity make filtering understandable. |
| C-16 | Status chips use **12/16** text, **24 minimum** visual height and **8** horizontal padding. Use a short label plus optional icon. A clickable chip gets a **44** target and explicit selected/remove behavior. | A status label should not pretend to be a button; a small button must not pretend to be harmless metadata. |
| C-17 | Distinguish healthy, warning, error, offline and simulated states by label and/or icon as well as color. Reserve **one persistent status area** per data source; show last update or age there. | “Live” is a data condition, not a decorative animation. |
| C-18 | Charts use stable units, explicit time windows and labeled series. For a percentage use a **0–100** axis; do not fill or interpolate before the first sample or across a known gap. Provide a text summary or data-table alternative. | Presentation must not fabricate history or rely on color discrimination. |

### 3.3 Empty, loading, error and alert states

| State | Required content and behavior | Reason |
| --- | --- | --- |
| No data yet | Region title, one sentence explaining what will populate it, and the relevant action if one exists. Preserve the planned chart/table height. | An empty region should explain the next step without collapsing the page. |
| No filter matches | State that filters returned no records; show a clear-filters action and the active filter summary. Do not present it as a system failure. | Zero results and broken retrieval require different user responses. |
| Initial loading | Before **300 ms**, keep the reserved region without flashing a spinner; after **300 ms**, show a labeled progress state or geometry-matched skeleton. Beyond **10 seconds**, show elapsed/slow status and a safe retry/cancel path when supported. | Short requests should not flicker, and long waits should not look abandoned. |
| Refreshing existing data | Keep the last confirmed content, mark it as updating/stale, and avoid remounting unrelated regions. | Existing evidence remains useful while retrieval continues. |
| Field error | Adjacent explanation, correction instruction and a semantic error cue; link the message to the field. | The user should know both what failed and how to fix it. |
| Region/network error | Name the failed task in plain language, retain safe prior content, display retry and last successful update where relevant. Never expose stack traces or infrastructure secrets in product copy. | Recovery requires actionable context, not internal implementation detail. |
| Live alert | Put a dismissible inline band below the page header and before the affected workspace. Use title, event meaning, time and scope. Keep critical/actionable alerts until dismissed or resolved; only noncritical confirmation toasts may auto-dismiss, default **6 seconds**. | Important conditions must not vanish before they are read or cover the controls used to respond. |
| Success | Confirm the completed object/action without shifting the form geometry. Clear only values the successful workflow explicitly consumes. | Feedback should preserve the user's context and avoid accidental data loss. |

## 4. Mobile behavior and fixed-element collision rules

### 4.1 Reflow and narrow tables

| ID | Rule and measurement | Reason |
| --- | --- | --- |
| M-01 | Reference modes are **below 640**, **640–1023**, and **1024+**. Test exact widths **320, 390, 768, 1024, 1440** and both sides of every chosen breakpoint. Component minimum widths, not device names, decide the final collapse point. | A breakpoint is valid only if content survives immediately before and after it. |
| M-02 | At **320 CSS px**, ordinary page content must reflow without horizontal page scrolling or lost actions. Allow at most **1 px** measured page overflow for rounding; fix greater overflow instead of hiding it globally. Two-dimensional data may use a contained scroll region. [W3C reflow guidance](https://www.w3.org/WAI/WCAG22/Understanding/reflow.html). | The page should remain usable with narrow viewports and enlarged content. |
| M-03 | For operational comparison tables, keep a real table inside a named, keyboard-reachable horizontal scroll region. Declare its minimum width from column budgets; for a log table start with **720**, with timestamp **128**, level **88**, service **160**, message **344**, including cell padding. Increase the budget when actions are added. | A table can preserve comparison without forcing the entire page sideways. |
| M-04 | Freeze at most **one identifier column**, width **104–128**, only when it is at most **40%** of the scroll viewport. Give frozen cells an opaque surface and a visible boundary; disable freezing if it obscures the remaining data. | A sticky column is useful only if enough comparison area remains. |
| M-05 | If users inspect each row independently, replace the narrow table presentation with stacked records: identifier/status first, essential value second, optional detail behind expansion. Keep column names as field labels and preserve every required action and full-value access. | Record cards support a reading task better than forced column alignment. |
| M-06 | Use one representation per viewport in the accessibility tree; inactive desktop/mobile duplicates are not focusable or announced. Do not solve narrow layouts by shrinking text below section 5's floor. | Duplicate controls and tiny text make an apparently responsive screen unusable. |
| M-07 | Stack filters at full width or in **two columns only when each remains 144 wide**. Keep filter labels and reset behavior. Give long forms their own scroll path; never require simultaneous horizontal and vertical form panning. | Inputs need usable targets and clear ownership on small screens. |

### 4.2 Fixed and sticky elements

| ID | Rule and measurement | Reason |
| --- | --- | --- |
| F-01 | Create one overlay inventory before implementation: element, anchor, dimensions, safe-area allowance, layer, scroll owner and conditions under which it is hidden. One owner controls each screen corner. | Independently positioned tools otherwise claim the same space. |
| F-02 | Use shared layer levels: content **0**, sticky content **10**, navigation **20**, popover **30**, noncritical toast **40**, modal backdrop **50**, modal **60**. Hide toasts/debug tools that would overlap an open modal. Do not use arbitrary values such as 9999. | A declared layer model prevents local components from escalating above unrelated interfaces. |
| F-03 | Prefer a single mobile bottom bar. If present, its content height is **64**, plus device bottom safe area **B**. A floating utility above it starts at least **64 + B + 16** from the viewport bottom. | Targets remain separated from navigation and device gesture areas. |
| F-04 | Measure the complete occupied fixed bottom stack as **H**, including its safe area. Reserve document bottom padding and scroll allowance of at least **H + 24**; for a sticky top header of height **T**, reserve top scroll clearance **T + 16**. Recalculate after wrapping, zoom or orientation changes. | Hardcoded offsets fail when labels wrap or text size changes. |
| F-05 | Keep at least **16** between separate floating rectangles and from viewport edges, in addition to safe-area insets. If the required stack exceeds **25% of visible viewport height**, move secondary utilities into normal flow or a drawer. | Multiple fixed controls must not consume the working area. |
| F-06 | Below **560 visible pixels of height**, or while the software keyboard obscures the lower workspace, suppress the debug launcher and relocate nonessential floating controls. Keep the active field, its error and the next form action scroll-reachable. | The keyboard leaves too little space for a permanent stack of utilities. |
| F-07 | Prefer inline alerts to fixed banners. If a toast is required, use width at most **360** on wide screens and viewport width minus **32** on narrow screens; stack at most **three**, then summarize. It must not cover the active field, submit, stop, close or pagination control. | Notifications must not obstruct the work they describe. |
| F-08 | A dialog fits within viewport height minus **32** and uses an internal scroll region when needed. Move focus into it, contain keyboard focus, provide a named close action, and return focus to its opener. Mobile bottom-sheet corners may differ only at the attached edge. | Visual containment must correspond to keyboard and scroll containment. |
| F-09 | Verify focused controls are fully visible under the house rule. WCAG 2.2 AA's minimum is that author-created content does not completely hide the focused component; this method is stricter. [W3C focus guidance](https://www.w3.org/WAI/WCAG22/Understanding/focus-not-obscured-minimum.html). | A fixed header or utility is not acceptable if it hides the current keyboard target. |

## 5. Measurement and accessibility gates

Use these as acceptance thresholds, not suggestions. Identity examples may choose a value inside the permitted family; changes outside it require a recorded reason and renewed checks.

| Property | Threshold or fixed default | Reason |
| --- | --- | --- |
| Normal text contrast | **At least 4.5:1**, including secondary text, placeholder and chip labels used to convey information. [W3C text contrast](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html). | Supporting text still has to be read. |
| Large text contrast | WCAG permits **3:1** for text at least **24 px regular**, or **18⅔ px bold** at the default pixel/point conversion. House preference remains **4.5:1** for operational values and headings. | Size can improve legibility, but critical data should not spend that margin unnecessarily. |
| Essential non-text cues | **At least 3:1** against adjacent colors for visual information needed to recognize controls, state or meaningful graph marks; inactive controls have standards exceptions. Decorative dividers are not control boundaries. [W3C non-text contrast](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html). | Faint decoration is acceptable; an invisible input or selected state is not. |
| Focus indicator | House rule: **2 px minimum** outline, **2 px offset**, and **3:1** against adjacent surfaces. Do not remove it in favor of a subtle color shift. | Keyboard location must remain unambiguous on every surface. |
| Pointer targets | House default **44 × 44**; explicit desktop density exception in C-02. WCAG 2.2 AA uses a **24 × 24** minimum with specified exceptions; 44 is this method's stronger default. [W3C target-size guidance](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html). | Workers must not confuse a standards floor with the chosen ergonomic target. |
| Type scale | Metadata **12/16**, dense body **14/20**, reading/input **16/24**, card title **20/28**, state heading **24/32**, mobile H1 **28/36**, desktop H1/metric **32/40**, optional editorial H1 **36/40**. Values are font size/line height. | A limited scale creates hierarchy without arbitrary per-component sizes. |
| Minimum essential type | **12 px metadata**, **14 px dense reading**, **16 px form input** at default size. Do not reproduce legacy 8–11 px labels for essential content. | High information density must not depend on unusually sharp eyesight. |
| Weights and tracking | Default UI weights **400, 500, 600, 700**. Uppercase is limited to short metadata labels of at most **24 characters**, with tracking **0.08–0.12 em**. Reading copy uses normal case and tracking. | Excessive capitals and letterspacing slow scanning and increase width. |
| Icon consistency | Use one icon family, **18–20 px** controls and **16 px** metadata; stroke default **1.5–2 px**, chosen consistently. A 16 px icon still sits in the required interaction target when clickable. | Optical consistency does not require shrinking the clickable area. |
| Form reading width | Single-column form maximum **560**; a compact side form is **320–400**. Field labels never truncate. | Long horizontal travel makes a simple form harder to complete. |
| Motion distances | Hover translation at most **2 px**; no hover scaling of dense table rows. Disclosure movement at most **8 px**. Avoid animating layout dimensions of the work region. | Motion should not move adjacent targets or disturb comparison. |
| Motion timing | Feedback **100–150 ms**, ordinary transitions **160–220 ms**, a dialog transition at most **240 ms**. Under reduced motion remove decorative loops, transforms and smooth scrolling; keep immediate state changes and static progress/status cues. | The interface acknowledges action without delaying completion or requiring motion tolerance. |
| Layout stability | Reserve image, chart and asynchronous panel geometry; pending controls keep their width; use tabular numerals for changing values. No unrelated region shifts when an action changes state. | Users need stable targets and a stable reading position. |
| Zoom and text | Check text at **200%** without missing content/actions; check reflow at a width equivalent to **320 CSS px**, including **1280 px viewport at 400% zoom** for ordinary vertical content. | Browser zoom exposes failures hidden by a single desktop screenshot. |
| Palette application | Action accent covers at most **10% of non-media interface area** in the reference screen, excluding meaningful chart marks and exceptional active alerts. Decorative background tint is at most **8%** over the base. | The action color loses significance if used as universal paint. |

WCAG references above support only the identified accessibility thresholds. Grid spans, palette coverage, radii, spacing, timing and minimum type choices are house design decisions; passing this checklist alone is not a claim of full WCAG conformance.

## 6. Twenty concrete signs of weak design

Treat these as review failures with observable evidence. Each row gives the corrective direction and the reason.

| # | Reject when this is observable | Correction and reason |
| --- | --- | --- |
| 1 | The identity is only “modern, clean, premium,” without audience, task or metaphor. | Rewrite using I-01; generic adjectives cannot resolve competing choices. |
| 2 | All three products would look the same after exchanging their logos. | Re-derive type, spacing, shape and primary composition, not just the accent hue; product behavior should drive identity. |
| 3 | Three or more equally filled action buttons compete in the initial task area. | Choose one primary and lower the others' emphasis; priority must be visible. |
| 4 | Success, selection, navigation and chart series all use an indistinguishable accent treatment. | Separate semantic roles and add labels; identical treatment implies identical meaning. |
| 5 | Every card has a gradient, glow, thick border and shadow. | Keep one structural separator and apply elevation only to actual layers; universal emphasis cancels hierarchy. |
| 6 | Adjacent components use unlisted spacing such as 13, 19 and 27 px without an optical exception. | Return to the shared spacing scale; irregular grouping makes the screen feel assembled from unrelated parts. |
| 7 | Essential labels are below 12 px or information-bearing text fails 4.5:1. | Increase size or contrast; faint miniature text is not a density strategy. |
| 8 | A desktop header consumes more vertical space than the initial working region, or a decorative hero pushes the main action below the first mobile viewport. | Reduce the header and prioritize the workspace; the product is for doing a task. |
| 9 | A side form is narrower than 320 px or the main evidence region is narrower than its declared minimum. | Stack or change placement; a nominal two-column layout is not a successful layout. |
| 10 | Form fields rely on placeholders, and errors appear only in a distant toast. | Restore persistent labels and adjacent correction messages; users should not have to remember context. |
| 11 | Segment labels wrap into multiple rows, or the entire visible segment cannot be activated. | Use a fitting segment group, vertical radios or a select; choices must remain comparable and operable. |
| 12 | A table appears to fit because its text was shrunk, key columns disappeared or long values cannot be expanded. | Use contained scrolling or labeled records with full-value access; apparent fit must not remove evidence. |
| 13 | The whole page pans horizontally at 390 or 320 px. | Find the overflowing element; constrain only the genuinely two-dimensional region; global overflow hiding conceals a defect. |
| 14 | A fixed debug button, toast or bottom bar overlaps submit, Stop, close or pagination. | Apply the shared collision inventory and measured offsets; utilities must not block the task. |
| 15 | Hover looks polished but keyboard focus is invisible, clipped or behind an overlay. | Restore the focus and visibility gates; pointer styling alone is incomplete interaction design. |
| 16 | “Live,” “Listening” or a moving waveform remains visible when the source is disconnected, idle or simulated without disclosure. | Render the actual state, source and last update; motion must not manufacture evidence. |
| 17 | An empty state is a blank hole, while a loading state briefly flashes “No results.” | Define separate empty/loading/filter/error contracts and preserve region geometry; different states require different explanations. |
| 18 | Cards jump, buttons change width or the page scroll position resets on routine refresh. | Reserve dimensions, preserve content and update in place; users should not chase moving targets. |
| 19 | Charts fill before the first observation, smooth through a known gap or rescale percentages to exaggerate changes. | Use truthful extents, gaps and stable units; a visual chart can mislead without a false numeric label. |
| 20 | Approval evidence is only a populated desktop screenshot. | Test empty, pending, failure, narrow, zoomed, keyboard and long-content cases; the ideal state is the easiest state to make attractive. |

## 7. Repeatable execution contract for an AI manager and workers

### 7.1 Required sequence and handoff

| Stage | Deliverable and exit condition | Reason |
| --- | --- | --- |
| 1. Understand | One identity sentence, three exclusions, ranked tasks, realistic content examples and a list of required data/source states. No token selection before these exist. | Design decisions need a task and truthful content to organize. |
| 2. Specify | One shared design contract containing semantic tokens, exact type/spacing/radius/elevation/motion values, contrast pairs, grid budgets and breakpoint/collision rules. | A worker cannot infer a coherent system from a mood description. |
| 3. Compose | A region list in reading order and a written primary/supporting hierarchy at **1440 × 1000** and **390 × 844**. Record the action-area decision using section 2.2. | Layout decisions must be settled before modules optimize their own appearance. |
| 4. Define states | A state matrix for each component: actual content, empty, pending, refreshing, failure, disabled, focus and success; include long labels and worst-case data. | State coverage is part of the design, not a final error-handling patch. |
| 5. Delegate narrowly | Give each worker its owned files/components, relevant rule IDs, shared token source, content/state examples, width budgets and exact completion checks. Workers report missing decisions instead of inventing local colors, spacing or layers. | Narrow ownership and shared contracts allow parallel work without visual divergence. |
| 6. Integrate | The manager checks shared edges, token reuse, dominant action, data truth and overlay ownership before approving isolated component screenshots. Changes to a token propagate to every consumer. | Locally attractive components can still form an incoherent screen. |
| 7. Verify | Produce the evidence below from the actual application; separate observed passes from untested assumptions. Fix failures and recapture the affected states. | A written rule becomes useful only when its implementation can fail review. |

### 7.2 Evidence required before design approval

| Check | Required evidence | Reason |
| --- | --- | --- |
| Identity | Each token family and the dominant region trace back to the identity sentence; exclusions are respected. | Prevents a generic template from passing merely because it is tidy. |
| Readability | A table of measured text, action, focus and essential graph/control contrast on their actual composited backgrounds, including selected and error states. | Readability cannot be established by visual confidence alone. |
| Width/height | Captures at **320, 390, 768, 1024 and 1440** wide; include a **390 × 568** short screen and a keyboard-open form check. No page overflow beyond M-02; no blocked target. | Tall desktop viewports conceal mobile collisions. |
| Long content | Exercise a **64-character service/name**, **120-character heading**, **500-character message**, large numeric values and a **200-character unbroken token** where such user data is permitted. Respect actual field limits rather than bypassing validation. | Real content tests wrapping, alignment and full-value access. |
| State completeness | Inspect empty, filtered-empty, initial loading, refresh with old data, failed request, pending action, confirmed success and disconnected/live source states. | State transitions often reveal stronger defects than the default layout. |
| Interaction | Complete the main task with keyboard only; verify meaningful control names, visible focus, segment behavior, dialog return focus and error recovery. | Appearance and operation must describe the same interface. |
| Preferences | Inspect **200% text/zoom**, narrow reflow and reduced motion. Ensure static status remains understandable without animation. | The identity must survive user preferences and constrained viewing. |
| Data integrity | Verify units, time window, sorting, simulation disclosure and chart extent against actual source data. | A visually convincing instrument must still tell the truth. |
| Human review | Show the owner at least one real desktop and one real mobile screenshot. Record the files inspected, defects fixed and any remaining limitations. Never claim a screenshot was viewed when it was only generated. | Review needs visible evidence and an honest boundary around verification. |

**Approval rule:** a design is complete only when the main task works, the identity is traceable, all applicable numeric gates pass and state/mobile evidence is available. A worker's completion message or a successful build does not replace these checks. **Reason:** compilation proves that an interface can run, not that its design decisions were implemented coherently.
