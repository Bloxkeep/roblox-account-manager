---
name: Bloxkeep
description: A quiet control room beside a wall of Roblox windows; one plain voice in English and Vietnamese.
colors:
  bg: "#0a0c11"
  bg-2: "#0d1017"
  surface: "#11151e"
  surface-2: "#161b26"
  surface-3: "#1d2331"
  raised: "#151a25"
  field: "#0b0e15"
  card: "#12161f"
  text: "#e9ecf3"
  text-2: "#b6bccc"
  muted: "#969cad"
  dim: "#697081"
  inv: "#e9ecf3"
  inv-ink: "#0b0d12"
  accent: "#22c55e"
  accent-ink: "#04200d"
  ok: "#34d399"
  online: "#60a5fa"
  warn: "#fbbf24"
  bad: "#fb7185"
  ok-ink: "#a7f3d0"
  warn-ink: "#fde68a"
  bad-ink: "#fecdd3"
typography:
  display:
    fontFamily: "Segoe UI Variable Display, Segoe UI, system-ui, sans-serif"
    fontSize: "28px"
    fontWeight: 700
    lineHeight: 1.15
  headline:
    fontFamily: "Segoe UI Variable Display, Segoe UI, system-ui, sans-serif"
    fontSize: "18px"
    fontWeight: 650
    lineHeight: 1.25
  title:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 650
    lineHeight: 1.35
  body:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.45
  label:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 600
    lineHeight: 1.3
  fine:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "11px"
    fontWeight: 500
    lineHeight: 1.3
  mono:
    fontFamily: "Cascadia Mono, Consolas, monospace"
    fontSize: "12px"
    fontWeight: 400
rounded:
  xs: "6px"
  sm: "9px"
  md: "11px"
  lg: "14px"
  xl: "22px"
  pill: "999px"
spacing:
  1: "4px"
  2: "6px"
  3: "8px"
  4: "10px"
  5: "12px"
  6: "16px"
  7: "20px"
  8: "24px"
  9: "32px"
components:
  button:
    backgroundColor: "{colors.surface-2}"
    textColor: "{colors.text}"
    typography: "{typography.title}"
    rounded: "{rounded.md}"
    height: "38px"
    padding: "0 14px"
  button-primary:
    backgroundColor: "{colors.inv}"
    textColor: "{colors.inv-ink}"
    typography: "{typography.title}"
    rounded: "{rounded.md}"
    height: "38px"
    padding: "0 14px"
  button-danger:
    backgroundColor: "{colors.bad}"
    textColor: "{colors.bad-ink}"
    rounded: "{rounded.md}"
    height: "38px"
  button-sm:
    rounded: "{rounded.sm}"
    height: "30px"
    padding: "0 10px"
    typography: "{typography.body}"
  button-go:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.accent-ink}"
    rounded: "{rounded.md}"
    height: "40px"
  field:
    backgroundColor: "{colors.field}"
    textColor: "{colors.text}"
    rounded: "{rounded.md}"
    height: "40px"
    padding: "0 12px"
  field-sm:
    rounded: "{rounded.sm}"
    height: "34px"
  chip:
    textColor: "{colors.text-2}"
    rounded: "{rounded.sm}"
    height: "30px"
    padding: "0 10px"
  chip-on:
    backgroundColor: "{colors.inv}"
    textColor: "{colors.inv-ink}"
  card:
    backgroundColor: "{colors.card}"
    rounded: "{rounded.lg}"
    padding: "16px"
  dialog:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.xl}"
  menu:
    backgroundColor: "{colors.raised}"
    rounded: "{rounded.lg}"
    padding: "6px"
  toast:
    backgroundColor: "{colors.raised}"
    rounded: "{rounded.lg}"
    padding: "11px 15px"
---

# Design System: Bloxkeep

The values above are the dark base theme (`:root` in `RobloxManager/wwwroot/app.css`). Every other theme overrides the
same token names; nothing in a component may name a colour that is not a token. The rules below hold on all ~90 themes,
on the native windows (`Themes.cs` carries the same roles) and on the macro panel beside the game.

## Overview

**Creative North Star: "The control room beside the farm"**

Bloxkeep is looked at, not lived in. The user has five to twenty Roblox windows open and glances at Bloxkeep to see
what needs them, then goes back to the game. So the surface is calm, dense where data is dense, and loud only where
something needs a hand: a kicked account, a stopped macro, a window Roblox closed. One sans family, solid colours, flat
panels separated by hairlines, and one green that means "go" and "alive" and nothing else.

The same voice runs everywhere: the main page, the tray panel, the dialogs, the macro panel floating beside a game, and
the few native windows. Nothing there is allowed to look like a different program. Vietnamese reads like a Vietnamese
app, short and natural, never like a translation.

**Key Characteristics:**
- Operate mode throughout: earned familiarity, no decoration, no motion that does not carry state.
- Solid colours only. No gradients, no glows, no blur as decoration (picture themes blur the backdrop, never the UI).
- One primary button look per surface; green is reserved for go/alive; red for stop/danger; amber for waiting/attention.
- Numbers always visible on charts and meters; a meter with no number is a bug.
- Short labels (1–4 words), the same word for the same thing everywhere (see Glossary).

## Colors

A restrained palette: layered neutrals from a theme's base, one accent, and a four-colour status vocabulary that stays
readable on every theme.

### Primary
- **Go green** (`--accent`, #22c55e on the base theme; each theme sets its own): the Play and Run buttons, the switch
  track when on, the selected calendar day, the live bar in charts. It means "this will act" or "this is alive". It is
  never a border for selection, never a heading colour, never a decoration.
- **Ink on green** (`--accent-ink`): text and icons on an accent fill.

### Neutral
- **Page** (`--bg`), **Panel** (`--surface`), **Raised** (`--raised`: menus, popovers, toasts), **Field** (`--field`:
  inputs, wells), **Card** (`--card`), and the hover steps (`--surface-2`, `--surface-3`). Depth is a step up the ladder,
  never a shadow.
- **Hairlines** (`--line` 6 % tint, `--line-2` 10 %, `--line-3` 17 %): dividers, card borders, field borders at rest.
- **Text** (`--text`), **Text 2** (`--text-2`: descriptions, row meta), **Muted** (`--muted`: hints, placeholders),
  **Dim** (`--dim`: disabled text).
- **Inverse** (`--inv` / `--inv-ink`): the primary button and the selected chip. White on dark themes, near-black on
  light ones. This is what "primary" looks like; it is not green.

### Status
- **OK** (`--ok`): playing, in game, done. **Online** (`--online`): online elsewhere, info notes. **Warn** (`--warn`):
  waiting, paused, needs attention, Roblox's bot check. **Bad** (`--bad`): kicked, stopped with an error, banned,
  destructive actions. Each has a soft fill (`--ok-soft`, `--warn-soft`, `--bad-soft`: 12–14 % of the colour) and an
  ink (`--ok-ink`…) for text on the soft fill.

### Named Rules
**The One Green Rule.** Accent green appears on at most one control per view (Play, Run, the switch that is on), on live
status dots and on live chart bars. Headings, borders, tabs and selected rows never use it.

**The Status Dot Rule.** A status colour is shown as a 7 px dot or a 4 px left rail plus a word, never as a coloured
background behind text and never as a coloured border around a card. A bot check is `warn`, not `ok`.

**The Solid Rule.** No `linear-gradient`, no `box-shadow` glow, no `filter: blur` on UI. The only shadows are the three
elevation shadows below.

## Typography

**Display Font:** Segoe UI Variable Display (falls back to Segoe UI, system-ui)
**Body Font:** Segoe UI Variable Text (falls back to Segoe UI, system-ui)
**Label/Mono Font:** Cascadia Mono, Consolas (job ids, cookies, keys typed by the user only; never for placeholders)

**Character:** the system face, tuned. Nothing says "designed"; everything reads at a glance at 150 % scaling.

### Hierarchy
- **Display** (700, 28px, 1.15): the page title only (Accounts, Presets, Macros…).
- **Headline** (650, 18px, 1.25): dialog titles, empty-state titles, Settings page titles.
- **Title** (650, 14px, 1.35): card names, settings row titles, section heads, buttons (600).
- **Body** (400, 13px, 1.45): everything else: descriptions, menus, chips, table cells, status lines.
- **Label** (600, 12px, 1.3): column heads, tab names, small chips, the "optional" tag. Mixed case; letter-spacing 0.
- **Fine** (500, 11px, 1.3): kbd hints, chip counts, fine print.

Seven sizes only: 11, 12, 13, 14, 15 (big button), 18, 28. Half-pixel sizes (12.5, 13.5…) are rounded to these.

### Named Rules
**The Mixed-Case Rule.** No uppercase + letter-spacing labels anywhere (the menu head and the panel heading drop
`text-transform`). Vietnamese diacritics and uppercase do not mix well, and the product's voice is spoken, not shouted.

**The Tabular Rule.** Any column of numbers (RAM, minutes, Robux, counters, times) sets `font-variant-numeric:
tabular-nums` so the column does not jitter as values change.

## Layout

- **Shell:** a 248 px sidebar (collapsible to 64 px) and a content column with a 24 px gutter; the Accounts dock is
  docked at the bottom of the content column and the content scrolls above it. The window is often 1000–1280 px wide;
  everything must work at 860 × 640 without wrapping a toolbar onto two rows.
- **Rhythm:** a 4 px base. Inside a card 16 px; between cards 12 px; between a title and its row 8 px; between
  sections 24 px. More space above a heading than below it.
- **Rows** (settings, accounts list, macros, presets): 56–64 px tall, title + description on the left, control on the
  right, a hairline between rows, never a card per row.
- **Toolbars never wrap.** The Accounts header is one row: title block (shrinks, ellipsis) · search · tools · Add. Chip
  rows shrink their labels before they hide anything; a chip that cannot fit becomes a "+2" chip, never a fade.
- **Dialogs:** 520 px default, 720 for the game page, 1120 for the macro editor; `max-height: calc(100vh - 40px)`; the
  body scrolls, head and foot do not.
- **Density:** lists may be dense (the accounts list, servers, activity); cards are not. Narrow (< 900 px) collapses the
  sidebar and turns the settings nav into one horizontally scrolling row, never three wrapped rows.

## Elevation & Depth

Tonal layering, not shadows. A surface sits a step up the neutral ladder (page → panel → raised) and is bounded by a
hairline. Shadows exist only to lift something off the page that was not there a moment ago.

### Shadow Vocabulary
- **Popover** (`box-shadow: 0 18px 50px var(--shadow)`): menus, pickers, the game picker.
- **Dialog** (`box-shadow: 0 30px 90px var(--shadow)`): modals and the lock card.
- **Toast** (`box-shadow: 0 14px 40px var(--shadow)`): toasts and the floating macro panel.

### Named Rules
**The Flat-At-Rest Rule.** Cards, rows, fields and buttons have no shadow at rest or on hover; hover is a one-step tonal
change (`--surface-2` → `--surface-3`, or `--hover` on transparent things).

## Shapes

Gently rounded, one family of radii, no pill buttons except pills. Corners scale with the size of the thing:

- **xs 6 px:** kbd, swatches, chip counts, checkboxes (5 px), small tags.
- **sm 9 px:** small buttons, chips, segmented controls, menu items, small fields (34 px).
- **md 11 px:** buttons (38 px), fields (40 px), notes, the number stepper.
- **lg 14 px:** cards, menus, popovers, toasts, the tray panel, the macro panel.
- **xl 22 px:** dialogs and the lock card.
- **pill:** status pills and the macro panel's folded "pill" mode only.

Borders are 1 px hairlines in `--line-2` (fields use `--line-field`, a stronger line, so they read as typeable).
Nothing is clipped diagonally; no outlined-only icons next to filled ones (one icon set, 16 px, 1.75 px stroke).

## Components

### Buttons
- **Shape:** md radius (11 px), 38 px tall, 14 px side padding, 8 px icon gap; `.sm` 30 px / 9 px / 10 px; `.big`
  48 px / 14 px radius / 15 px text.
- **Primary** (`.btn.primary`): `--inv` fill, `--inv-ink` text, no border. One per dialog or toolbar: the thing you
  came to do (Save, Add account, Verify, Import, Done). Green primaries (`.btn.go`) are gone from the page; the dock's
  Play and the macro panel's Run are the only accent-filled buttons in the product.
- **Secondary** (`.btn`): `--surface-2` fill, `--line-2` border. Any other action.
- **Ghost** (`.btn.ghost`): transparent, hover `--hover`. Cancel, Back, "Later".
- **Danger** (`.btn.danger`): `--bad-soft` fill, `--bad-line` border, `--bad-ink` text. In a confirm dialog it sits
  right as the primary action; in an editor (Delete preset, Delete macro) it sits left as a ghost danger.
- **Go** (`.btn.go`, allowed only on the dock Play and the panel Run): `--accent` fill, `--accent-ink` text, 40 px.
  While running it becomes **Stop**: `.btn.danger` look, same size, so the colour says "this stops something".
- **Icon button** (`.icon-btn`): 34 × 34, 10 px radius, transparent, `--muted` icon → `--text` on hover.
- **Hover / Active / Disabled:** one tonal step up on hover; `translateY(1px)` on press; disabled `opacity: .45` and a
  `title` saying why (a disabled primary never carries an instruction as its label).
- **Focus:** `:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px }`. Never a box-shadow ring on a
  button, never a ring after a mouse click.

### Chips (filters and view switches)
- **Filter chip** (`.chip`): 30 px, sm radius, transparent, `--text-2`, a 7 px status dot where it filters by status,
  a count in a `--active` pill. Selected: `--inv` fill with `--inv-ink` text (the primary look, not green).
- **Segmented control** (`.seg`): one component for every "pick a view" pair (Stats | Activity, Alerts | Status
  message, Roblox | Roblox VNG, card | list). 30 px in toolbars, 38 px `.seg.wide` in forms. Selected `--surface-3`.
  Chips are for filtering a list; segments are for switching what is shown. Never both for the same job.

### Cards / Containers
- **Corner Style:** lg (14 px). **Background:** `--card`, hover `--card-hover`. **Border:** 1 px `--line`; a card that
  is selected gets `--line-3` plus the checkbox, never an accent border; a local (this PC) card gets a 6 % ok tint via
  `color-mix`, no gradient. **Padding:** 16 px. **Shadow:** none.
- **Settings group:** a `--surface` box with lg radius holding rows separated by `--line`; the group title sits above
  the box at Title size.

### Inputs / Fields
- **Field** (`.field`): 40 px, md radius, `--field` fill, 1 px `--line-field`; `.sm` 34 px / sm radius. Placeholder
  `--muted` in the UI face (never mono). Textareas have no resize grip and grow with their content (max 6 lines).
- **Focus:** border `--focus-line` + `0 0 0 3px var(--focus-ring)` (fields are the one place a soft ring is allowed).
- **Error:** `aria-invalid="true"` → border `--bad-line`; the message sits under the field in Body size, `--bad-ink`,
  and clears as soon as the user types. Never a bare red sentence with no field marked.
- **Disabled:** `opacity: .55`, `--bg-2` fill, `cursor: not-allowed`.
- **Number:** Chromium spin buttons hidden everywhere (`::-webkit-inner-spin-button { appearance: none }`). Where a
  number is changed often (macro panel, hours) it is a **stepper**: `[−] [ value unit ] [+]`, 34 px, the value typeable,
  hold-to-repeat on the buttons (400 ms then every 40 ms; Shift × 10), disabled at min and max.
- **Checkbox:** custom, 16 px, 5 px radius, `--field` fill, `--line-3` border; checked `--accent` fill with an
  `--accent-ink` tick; focus ring as buttons. No native checkbox anywhere.
- **Switch** (`.switch`): 42 × 24, `--surface-3` track, `--knob` knob; on: `--accent` track, `--accent-ink` knob. A
  switch is for a setting that takes effect at once; a checkbox is for an option inside a form or a list to tick.
- **Select:** the app's styled select (`.select`, chevron from `--chev`, `appearance: base-select` where Chromium has
  it) with the popup drawn by the page; `<optgroup>` for grouped choices. Never the OS popup.
- **Key field:** a kbd box plus "Change" → the box reads "Press a key…"; Esc cancels; a refused key says "Can't use this
  key" under the row for 3 s.
- **Colour field** (`ColorField`): swatch + hex + Pick; the swatch is not a button. **Time / date** (`TimeField`):
  text box + the app's own popover; 24 h and D/M in Vietnamese, the system's in English; no native segments.

### Notes (info / warning / error)
One component, three colours: `.note` 13 px, 10 px 12 px padding, md radius, a 16 px icon, soft fill + ink colour
(`--online` info, `--warn` warning, `--bad` error). Used for the aspect warning, the Fast Flags note, the
"Roblox closed windows" warning, a failed list ("Couldn't load servers" + Retry). Never yellow text without a box, never
a bordered box with a different padding elsewhere.

### Dialogs
- `.modal`: xl radius, `--surface`, 1 px `--line-2`, dialog shadow; width per Layout; `max-height: calc(100vh - 40px)`;
  `display: flex; flex-direction: column`.
- **Head:** title (Headline) left-aligned always, close ✕ right, hairline under it. **Body:** scrolls, 8 px 24 px.
  **Foot:** hairline above, right-aligned actions, 16 px 24 px 22 px; destructive secondary at the left.
- Content never slides under the foot: the hairline plus body padding-bottom 8 px make the cut edge honest.
- A dialog that reports success shows one banner (`.note` ok) and no toast; a Done primary closes it.
- "Don't ask again" is a checkbox row under the body text, never on a dialog whose loss is data (discarding edits).

### Menus and popovers
`.menu`: raised fill, lg radius, 6 px padding, 36 px items, sm radius hover; groups separated by a hairline; the
danger item last; a check icon marks the current choice (no per-row decorative icons). Anchored to the opener's
rectangle, flipped only when it would overflow; a second click on the opener closes it. Items that cannot apply are
hidden, not greyed, unless the greyed item carries a `title` saying why.

### Toasts
Bottom-right, 16 px from the edges, above the dock on the Accounts view (`bottom: var(--dock-h) + 16px`), at most three
stacked, 4 s (6 s for errors), one line of Body text with a 16 px status icon, optional one action ("Undo"). A toast is
never shown for a result the open dialog already shows.

### Empty, loading and error states
- **Empty:** a 72–88 px art, Headline title, one Body sentence, one primary action. Period chips, search boxes and
  counts are hidden in an empty state.
- **Loading:** skeleton rows (`.skel`) in the shape of the list (4 rows), never a lone spinner in a void; a button that
  works shows `.busy` on itself.
- **Error:** a `.note.bad` in place of the list with the reason in plain words and a Retry button.

### Charts and meters
Every bar carries its value (or the top value and today's when there are 30), a solid bar colour (45 % accent mix
at rest, accent for today, 70 % on hover), one continuous baseline, and the average in the title. A meter with no
number beside it is removed.

### Signature component: the macro panel
A 300 px floating window beside the game, drawn by the same React component the editor shows as "Player sees", in the
app's theme. Title row (name · Beta tag · fold · close) · the Go button Run with its hotkey · a status line with a dot
· tabs (segmented) · setting rows (label left, control right) · foot (Edit areas · Reset all). Folded, it is a 300 ×
40 pill: dot, name, Run/Stop, unfold. Full spec in the redesign plan, package P1.

## Do's and Don'ts

### Do:
- **Do** use `.btn.primary` for the one main action of a dialog or toolbar and nothing else.
- **Do** mark the offending field (`aria-invalid`) and say the fix in one sentence under it.
- **Do** put a `title` on every disabled control that says why it is disabled.
- **Do** show numbers on bars, counts on chips, "0" or "—" on empty days.
- **Do** keep toolbars on one row at 860 px; shorten labels before you wrap.
- **Do** use the same word for the same thing (Glossary) and the same component for the same job (chips filter,
  segments switch, switches act at once, checkboxes tick).
- **Do** close a popup on the second click of its opener, and anchor it to the opener.
- **Do** write Vietnamese in 1–4-word labels, 24 h clock, D/M dates, decimal comma.

### Don't:
- **Don't** use gradients, glows, blur on UI, uppercase letter-spaced labels, or display sizes inside components.
- **Don't** use green for selection, borders, tabs or headings; green is Play/Run/on/alive only.
- **Don't** use any native widget: no OS select popup, date/time/colour picker, checkbox, number spinner, textarea grip,
  `<details>` marker, stock tooltip, or system scrollbar on a dark native window.
- **Don't** show the same message twice (banner + toast, inline + summary, two "locked" lines).
- **Don't** put an instruction in a disabled button's label ("Select accounts first"); keep the verb, hint in `title`.
- **Don't** let a label be cut mid-word without an ellipsis, or a status column truncate while a less important column
  keeps its width.
- **Don't** render anything from the test mock's English in a Vietnamese screenshot and call it a bug (see the plan's
  mock-only list).

## Wording

Rules for every string in `i18n.js`, `L.cs` and the native windows:

1. **Short.** Labels 1–4 words; descriptions one sentence; errors one sentence that names the fix.
2. **Spoken, second person.** "Open Roblox first", "Mở Roblox trước". Never "I" in the UI ("When you press Run", not
   "When I run it"). No scolding ("Đừng tắt…" → "Giữ Bloxkeep mở khi chơi.").
3. **No jargon in user text.** No PID, build ids, ms, px, "variable", "tolerance", "launcher name" in a sentence. Units
   are words or symbols the player knows: "0.6 s" / "0,6 giây", "%", "px" only where a pixel is the thing.
4. **US spelling in English** (color, minimize, customize). Code identifiers keep whatever they have.
5. **Vietnamese:** no "acc"; "tài khoản"; "Preset" capitalised as a product word; numbers "1,7 GB", "3,1 giờ"; time
   "20:00"; dates "16/10/2026"; weekdays "T2–T6", "CN" in summaries ("Thứ 5, 24/12" in headings); calendar head
   "Tháng 10, 2026".
6. **Durations:** long form "2 h 20 min" / "2 giờ 20 phút"; short form on bars and chips "2h20" in both languages.
7. **Capitalisation:** sentence case everywhere; a mid-sentence "Nếu"/"If" is lower case.
8. **Plurals:** English has `…1` keys; Vietnamese has none, and a missing VI key falls back to the VI plural key, never
   to English.
9. **One name per thing:** see the Glossary. A new string that introduces a second name for an existing thing is a bug.

## Glossary

| Thing | EN | VI | Notes |
|---|---|---|---|
| account | account | tài khoản | never "acc" |
| preset | preset / Preset (title) | Preset | VI always capital P |
| macro | macro | macro | |
| the panel beside the game | panel | bảng điều khiển | button label "Panel" / "Bảng điều khiển"; in a sentence "bảng" |
| theme | theme | chủ đề | never "theme" in VI |
| appearance (settings page) | Appearance | Giao diện | |
| drop (left the game unexpectedly) | drop, dropped, drops | văng, bị văng, lần văng | tile "Drops" / "Lần văng"; column "Drops" / "Văng"; card "Why they dropped" / "Vì sao văng"; filter "Drops and errors" / "Văng và lỗi" |
| network reason for a drop | lost connection | rớt mạng | only as a reason, never for the event |
| idle kick | kicked for AFK | bị kick vì AFK | everywhere, including Statistics |
| Anti-AFK | Anti-AFK | Chống AFK | |
| leaving windows open to farm | AFK / hanging | treo máy | never "treo game" |
| rejoin | rejoin, rejoining… | vào lại, đang vào lại… | "Cancel rejoin" / "Hủy vào lại" |
| sign in again | needs sign-in | cần đăng nhập | never "cần đăng nhập lại" on chips |
| bot check | Roblox asks for a check | Roblox yêu cầu xác minh | never "đòi" |
| window | window | cửa sổ | "+2 more windows" / "+2 cửa sổ khác" |
| vault (the account store) | vault | vault | multi-vault concept only |
| the .rmv file | Bloxkeep file | file Bloxkeep | "Import from a Bloxkeep file" / "Nhập từ file Bloxkeep" |
| server | server | server | |
| job id | Job ID | Job ID | |
| private server code | private server code | mã server riêng | |
| Multi-Roblox | Multi-Roblox | Multi-Roblox | status "On" / "Đang bật", "Off" / "Đang tắt", "Not working yet" / "Chưa chạy được" |
| game monitor | Game monitor | Theo dõi game | |
| statistics | Statistics | Thống kê | |
| settings | Settings | Cài đặt | never "tùy chọn" for the page |
| a macro's player setting | setting | cài đặt | never "tùy chọn" |
| area (box on the game) | area | vùng | |
| point / spot | spot | điểm | "Pick spot" / "Chọn điểm"; a step's dot is "point 4" / "điểm 4" |
| counter | counter | bộ đếm | shown "fish: 3" |
| switch (on/off item) | Switch | Công tắc | |
| one-of list | Pick one | Chọn một | |
| several-of list | Pick several | Chọn nhiều | |
| heading on the panel | Heading | Tiêu đề | |
| tab | tab | tab | |
| Run / Stop | Run / Stop | Chạy / Dừng | |
| panel ready state | Ready | Sẵn sàng | never "Idle" / "Đang nghỉ" |
| no game for the panel | Open Roblox first | Mở Roblox trước | |
| stopped with error | Stopped: … | Đã dừng: … | |
| Try it | Try it | Chạy thử | |
| Record | Record | Ghi | |
| Fix (open the failing step) | Fix | Sửa lỗi | |
| Reset all panel settings | Reset all | Đặt lại hết | |
| Edit areas on the game | Move areas on game / Stop editing | Chỉnh vùng trên game / Xong | |
| Save / Cancel / Done / Close | Save / Cancel / Done / Close | Lưu / Hủy / Xong / Đóng | |
| discard edits dialog | Keep editing / Discard changes | Sửa tiếp / Bỏ thay đổi | |
| delete | Delete | Xóa | |
| last opened sort | Last opened | Mở gần nhất | |
| layout auto | Auto | Tự động | never "Tự vừa" |
| palette category for pages | Page | Trang | never "Bảng" |
| quiet hours | quiet hours | giờ yên tĩnh | |
| Discord webhook link | link | link | "Change link…" / "Đổi link…" |
| every few minutes trigger | Every few minutes | Định kỳ | never "Again and again" / "Lặp lại" |
| close all | Close all Roblox? | Đóng hết Roblox? | body "3 windows will close." / "Sẽ đóng 3 cửa sổ." |
| tray "new Roblox" tile | Sign-in window | Cửa sổ đăng nhập | |
