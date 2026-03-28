<!--
 SPDX-FileCopyrightText: 2026 HUIHONG YOU
 SPDX-License-Identifier: GPL-3.0-or-later
-->

# UI Mockups — SoymilkyTracker

Pixel-art tracker UI modelled closely on MilkyTracker's classic layout and
colour palette.  The goal is to give experienced MilkyTracker users an
immediately familiar environment while removing the need to memorise commands.

All measurements are derived from MilkyTracker's source
(`src/tracker/TrackerInit.cpp`, `PatternEditorControl.cpp`,
`TrackerConfig.cpp`).  Reference resolution: **800 × 600 px**.

---

## 1. Colour Palette (Classic)

Sourced from `TrackerConfig.cpp` default values and the built-in
`predefinedColorPalettes[1]` ("classic") preset.

| Role | Hex | R,G,B | Usage |
|---|---|---|---|
| Pattern background | `#000000` | 0,0,0 | Pattern editor fill |
| Note text | `#FFFFFF` | 255,255,255 | Note column (C-4, F#3 …) |
| Instrument text | `#80E0FF` | 128,224,255 | Instrument digits |
| Volume text | `#80FF80` | 128,255,128 | Volume column |
| Effect letter | `#FF80E0` | 255,128,224 | Effect type character |
| Effect operand | `#FFE080` | 255,224,128 | Effect parameter digits |
| Empty cell (`·`) | `#404040` | 64,64,64 | Unfilled slots |
| Cursor cell | `#8080FF` | 128,128,255 | Current cursor position |
| Cursor line | `#602040` | 96,32,64 | Current row highlight band |
| Cursor line (record) | `#A01830` | 160,24,48 | Record-mode row colour |
| Selection block | `#103060` | 16,48,96 | Marked block background |
| Primary beat row bg | `#202020` | 32,32,32 | Every 4th row |
| Secondary beat row bg | `#101010` | 16,16,16 | Every 8th row |
| Primary beat text | `#FFFF00` | 255,255,0 | Row number, every 4th row |
| Secondary beat text | `#FFFF80` | 255,255,128 | Row number, every 8th row |
| Normal row text | `#FFFFFF` | 255,255,255 | Row number, other rows |
| Desktop / chrome | `#406080` | 64,96,128 | Borders, panel chrome |
| List box background | `#202030` | 32,32,48 | Instrument/sample lists |
| Scopes background | `#202030` | 32,32,48 | Channel oscilloscopes |
| Scopes waveform | `#FFFFFF` | 255,255,255 | Oscilloscope lines |
| Sample waveform | `#FFFF80` | 255,255,128 | Sample editor waveform |
| Muted channel text | `#808080` | 128,128,128 | Muted channel label |
| Button background | `#C0C0C0` | 192,192,192 | All push-buttons |
| Button text | `#000000` | 0,0,0 | Button labels |
| Record button text | `#FF0000` | 255,0,0 | Record toggle active |
| Static label text | `#FFFFFF` | 255,255,255 | UI labels |
| Scrollbar track | `#203040` | 32,48,64 | Scrollbar background |

In egui these map directly to `egui::Color32::from_rgb(R, G, B)`.

---

## 2. Typography

SoymilkyTracker uses a single vendored bitmap font for all UI text.
All text is rendered at pixel-exact positions with no anti-aliasing.

| Role | Font | Size | File |
|---|---|---|---|
| **Primary UI — everything** | **IBM EGA 8×8 (Ac437)** | **8 × 8 px** | `assets/fonts/Ac437_IBM_EGA_8x8.ttf` |

**IBM EGA 8×8** is a pixel-perfect TTF reproduction of the IBM PC EGA BIOS
ROM font, taken from the *Ultimate Oldschool PC Font Pack v2.2* by VileR
(CC BY 4.0, `https://int10h.org/oldschool-pc-fonts/`).  It covers IBM Code
Page 437 (Latin + box-drawing + block-element glyphs) — sufficient for all
tracker UI text.  The font is registered in egui under the family name
`"tracker"` (see `install_fonts()` in `crates/tracker-client/src/app.rs`).

Sizing convention: egui `FontId::new(8.0, FontFamily::Name("tracker".into()))`.
At native (1×) scale this gives 8 px tall glyphs matching the 8-px row height
used in all column layout calculations below.

**Empty-cell placeholder:** render as `·` (U+00B7 MIDDLE DOT) in colour
`#404040`, or a 1×3 px filled rect centred in the cell.

---

## 3. Overall Layout (800 × 600)

Panels are stacked vertically.  Heights are fixed; the pattern editor expands
to fill remaining vertical space.

```
 x=0                                                              x=800
 ┌────────────────────────────────────────────────────────────────────┐  y=0
 │  TITLE BAR                                             h=24        │
 ├──────────────────────────────────────────────────────┬─────────────┤  y=24
 │  ORDER LIST │ SPEED SECTION   │ PATTERN SECTION       │            │
 │  w=114      │ w=97            │ w=109                 │  h=40      │
 ├──────────────────────────────────────────────────────┤            │  y=64
 │  MENU BUTTONS (4 × 4 grid of 77×11 px buttons)       │  h=54      │
 ├──────────────────────────────────────────────────────┴─────────────┤  y=118
 │  INSTRUMENT LIST (left ~50%)  │  SAMPLE LIST (right ~50%)          │
 │                                                         h=118      │
 ├────────────────────────────────────────────────────────────────────┤  y=236
 │  CHANNEL SCOPES (one oscilloscope per channel)          h=64       │
 ├────────────────────────────────────────────────────────────────────┤  y=300
 │                                                                    │
 │  PATTERN EDITOR                                                    │
 │                                          fills remaining height    │
 │                                                                    │
 ├────────────────────────────────────────────────────────────────────┤  y=584
 │  TAB BAR                                                h=16       │
 └────────────────────────────────────────────────────────────────────┘  y=600
```

The **Sample Editor** and **Instrument Editor** panels are overlays that
replace the bottom portion of the pattern editor when activated.  They are
not separate windows.

---

## 4. Title Bar (h = 24)

```
┌────────────────────────────────────────────────────────────────────────────┐
│ [Title: My_Module_Name_____________] │[F][P][W][L]│ Time │  Peak  │       │
└────────────────────────────────────────────────────────────────────────────┘
```

- **Song title** — editable text field, ~200 px wide.
- **F** (Follow song), **P** (Prospective), **W** (Wrap cursor),
  **L** (Live) — 12×12 px toggle buttons, lit when active.
- **Time / Peak** — 30 px toggle buttons; switch the right readout between
  elapsed time and peak meter.

---

## 5. Controls Row (h = 40, three sections)

### 5a. Order List (x=0, w=114)

```
┌──────────────────────────────────────────────────────┐
│ ORDER                                                 │
│ ┌──────────────────────┐  [Ins.][Add]                │
│ │▶00  00               │  [ + ][ - ]                 │
│ │  01  01              │  [Del]                      │
│ │  02  02              │                             │
│ │  03  03              │  Len: 08  [ + ][ - ]        │
│ │  04  00              │  Rep: 00  [ + ][ - ]        │
│ └──────────────────────┘                             │
└──────────────────────────────────────────────────────┘
```

- A vertical **scrolling list** showing hex pattern indices.
- Each entry: `[position_hex]  [pattern_hex]`
  - e.g., `▶00  02` means position 0 plays pattern 02; `▶` marks playback pos.
- Buttons to the right: **Ins.** (insert), **Add**, **+/−** (inc/dec pattern
  number), **Del** (delete position), **Len +/−** (song length),
  **Rep +/−** (restart point).

### 5b. Speed Section (x=114, w=97)

```
┌──────────────────────────────────┐
│ BPM [125] [+][-]                 │
│ TPB [  6] [+][-]                 │
│ Step[  1] [+][-]  Oct [4] [+][-] │
└──────────────────────────────────┘
```

- **BPM** — beats per minute (1–255).
- **TPB** — ticks per beat / speed (1–31).
- **Step** — cursor advance after note entry (0 = no advance).
- **Oct** — current octave for keyboard note input (0–8).

### 5c. Pattern Section (x=211, w=109)

```
┌────────────────────────────────────┐
│ Pat  [00] [+][-]                   │
│ Rows [ 40] [+][-]   [*] [/]        │
└────────────────────────────────────┘
```

- **Pat** — current pattern index (hex).
- **Rows** — row count of current pattern (1–256).
- **\*** Expand (double length), **/** Shrink (halve length).

---

## 6. Menu Buttons (h = 54, 4 × 4 grid of 77 × 11 px)

```
┌───────────┬───────────┬───────────┬───────────┐
│ Play Song │ Play Pat  │   Stop    │    Zap    │
├───────────┼───────────┼───────────┼───────────┤
│   Load    │   Save    │  Disk Op. │  Ins. Ed. │
├───────────┼───────────┼───────────┼───────────┤
│  Smp. Ed. │ Adv. Edit │ Transpose │  Config   │
├───────────┼───────────┼───────────┼───────────┤
│  Options  │ Optimize  │   About   │  + / − Ch │
└───────────┴───────────┴───────────┴───────────┘
```

- **Play Song** — play from current order-list position.
- **Play Pat** — loop current pattern.
- **Stop** — stop playback.
- **Zap** — clear everything (with confirmation).
- Buttons are `#C0C0C0` background, `#000000` text, flat pixel border.
- **Rec** (record) state: "Stop" button text becomes red (`#FF0000`).

---

## 7. Instrument & Sample Lists (h = 118)

```
┌─────────────────────────────────────┬──────────────────────────────────────┐
│ Instruments         [+][-][%][Zap]  │ Samples             [Clr][Load][Save]│
│ [Load] [Save]                       │                                      │
│ ┌────────────────────────────────┐  │ ┌───────────────────────────────────┐ │
│ │▶01  Lead Synth                 │  │ │▶00  Lead wave                     │ │
│ │  02  Bass Line                 │  │ │  01  Loop section                 │ │
│ │  03  Kick Drum                 │  │ │  02  (empty)                      │ │
│ │  04  Snare                     │  │ │  03  (empty)                      │ │
│ │  05  Hi-Hat                    │  │ │                                   │ │
│ │  06  (empty)                   │  │ │                                   │ │
│ └────────────────────────────────┘  │ └───────────────────────────────────┘ │
└─────────────────────────────────────┴──────────────────────────────────────┘
```

- **Instrument list** (left half): each entry = `index  name`; index in
  `#80E0FF`, name in `#FFFFFF`.  `▶` marks the currently selected instrument.
- **Sample list** (right half): per-instrument sample slots; `▶` marks active.
- Both lists: `#202030` background, `#406080` border.
- **In-place name editing** on double-click / Enter.

---

## 8. Channel Scopes (h = 64)

```
┌────────────────────────────────────────────────────────────────────┐
│  CH 1                │  CH 2                │  CH 3                │  …  │
│  ──────╮╰──────────  │  ────────────────    │  ─────╮╰────────     │     │
│        ╰──────────   │                      │       ╰────────      │     │
│  [M][S][R]           │  [M][S][R]           │  [M][S][R]           │     │
└────────────────────────────────────────────────────────────────────┘
```

- One oscilloscope per channel — real-time waveform preview.
- Background `#202030`, waveform `#FFFFFF`.
- **[M]** Mute, **[S]** Solo, **[R]** Rec — tiny 10×10 px toggle buttons per
  channel below the scope display.
- Muted channels: waveform dim (`#404040`), channel header text `#808080`.

---

## 9. Pattern Editor

This is the core of the application.  It fills the remaining vertical space
(~284 px at 800×600, showing ~35 rows at 8-px font).

### 9a. Column Layout (per channel, at IBM EGA 8×8)

```
Slot width = 10 × 8 + 7 = 87 px per channel

 offset 0          24 25      33 34      42 43     51 52       60 61      69 70      78
         ┌──────────┐ ┌────────┐ ┌────────┐ ┌───────┐ ┌────────┐ ┌────────┐ ┌────────┐
         │  Note    │ │ Ins Hi │ │ Ins Lo │ │Vol Hi │ │ Vol Lo │ │ Fx Ltr │ │ Op Hi  │ Op Lo │
         │  3 chars │1│ 1 char │1│ 1 char │1│ 1 chr │1│ 1 char │1│ 1 char │1│ 1 char │1chr   │
         └──────────┘ └────────┘ └────────┘ └───────┘ └────────┘ └────────┘ └────────┘
```

Each `│` separator is 1 px wide.  Colour per field:

| Field | Colour | Example |
|---|---|---|
| Note | `#FFFFFF` | `C-4`, `F#5`, `B-3`, `···` |
| Instrument | `#80E0FF` | `01`, `··` |
| Volume | `#80FF80` | `40`, `··` |
| Effect letter | `#FF80E0` | `A`, `·` |
| Effect operand | `#FFE080` | `00`, `··` |
| Empty placeholder | `#404040` | `···`, `··` |

### 9b. Row Structure

```
       CH 1                   CH 2                   CH 3
row ┌──────────────────────┬──────────────────────┬──────────────────────┐
 00 │ ··· ·· ·· ···        │ ··· ·· ·· ···        │ ··· ·· ·· ···        │
 01 │ ··· ·· ·· ···        │ ··· ·· ·· ···        │ ··· ·· ·· ···        │
 02 │ ··· ·· ·· ···        │ ··· ·· ·· ···        │ ··· ·· ·· ···        │
 03 │ ··· ·· ·· ···        │ ··· ·· ·· ···        │ ··· ·· ·· ···        │
 04 │ C-4 01 40 ···        │ ··· ·· ·· ···        │ E-4 03 30 A04        │  ← beat row
 05 │ ··· ·· ·· ···        │ G-4 01 ·· E08        │ ··· ·· ·· ···        │
 06 │ ··· ·· ·· ···        │ ··· ·· ·· ···        │ ··· ·· ·· ···        │
 07 │ ··· ·· ·· ···        │ ··· ·· ·· ···        │ ··· ·· ·· ···        │
 08 │ C-4 01 ·· ···        │ ··· ·· ·· ···        │ F-4 03 ·· ···        │  ← 2°beat
═══ ╞══════════════════════╪══════════════════════╪══════════════════════╡  ← cursor
 09 │ ··· ·· ·· ···        │ A-4 02 30 ···        │ ··· ·· ·· ···        │
 0A │ ··· ·· ·· ···        │ ··· ·· ·· ···        │ ··· ·· ·· ···        │
    └──────────────────────┴──────────────────────┴──────────────────────┘
```

- **Row number** column (2 hex digits wide = 16 px): left of the grid.
  - Normal rows: `#FFFFFF` text, `#000000` background.
  - Primary beat (every 4 rows): `#FFFF00` text, `#202020` background.
  - Secondary beat (every 8 rows): `#FFFF80` text, `#101010` background.
- **Cursor row** (`═══`): full-width horizontal band, `#602040` background.
  In record mode: `#A01830`.
- **Cursor cell**: `#8080FF` background highlight on the focused sub-column.
- **Playback position row**: 50%-brightness `#406080` → `#203040` overlay.
- **Channel separator**: 3 px wide groove lines using chrome colour derivatives.

### 9c. Channel Header Row (h = 12)

```
┌──────────────────────┬──────────────────────┬──────────────────────┬───
│         1            │         2            │         3            │  …
└──────────────────────┴──────────────────────┴──────────────────────┴───
```

- Shaded gradient box per channel (lighter at top, darker at bottom) using
  `#406080` derivatives.
- Muted channels: text `#808080`, label appended with `(Mute)`.
- Click to mute/unmute; right-click to solo.

### 9d. Scrollbars

- 8 px wide, `#203040` track, `#406080` thumb.
- Horizontal scrollbar at bottom of pattern editor (for channels > viewport).
- Vertical scrollbar at right (for rows).

---

## 10. Sample Editor (overlay, replaces lower portion when active)

```
┌────────────────────────────────────────────────────────────────────┐
│  SAMPLE EDITOR — "Lead wave"                                       │
│ ┌──────────────────────────────────────────────────────────────┐  │
│ │                                                              │  │
│ │ ···~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~· │  │
│ │ ~~─────────────────╮╰────────────────────────────────────── │  │
│ │                     ╰────╮╰───────────────────────────────── │  │
│ │ ──────────────────────────╰───────────────────────────────── │  │
│ │ ···                                                          │  │
│ │             [  ]                              [  ]           │  │
│ │           Loop St                           Loop End         │  │
│ └──────────────────────────────────────────────────────────────┘  │
│                                                                    │
│ [Synth][Stop][↑][↓] │[Show][All][Undo][Redo][Zoom─][Show All]     │
│ [Cut][Copy][Paste]  │[Crop][Vol.][Draw] │ ◉NoLoop ○Fwd ○Ping-Pong│
│ [Load][Save][Exit]  │ Len:00400 Rep:00000 RepLen:00000            │
└────────────────────────────────────────────────────────────────────┘
```

- **Waveform area**: `#000000` background, `#FFFF80` waveform lines.
- **Loop start / end markers**: vertical dashed lines in `#FF80E0`
  (draggable; snap to sample frame boundaries).
- **Selection range**: `#103060` overlay within the waveform.
- **Loop type radio**: `◉ No loop`, `○ Forward`, `○ Ping-pong`.
- Bottom toolbar split into 7 sections (see Section 7 of source research
  for exact pixel widths at 800 px wide screen).

---

## 11. Instrument Editor (overlay, replaces lower portion when active)

```
┌────────────────────────────────────────────────────────────────────┐
│  INSTRUMENT EDITOR — "Lead Synth"                                  │
│                                                                    │
│  ┌─ Volume Envelope ──────────────────┐  ┌─ Panning Envelope ───┐ │
│  │  64 ┤         ╭──────╮             │  │  64 ┤ ─────────────   │ │
│  │     │        ╱        ╲            │  │     │                 │ │
│  │   0 ┤───────╯          ╲───────    │  │  32 ┤ ─────────────   │ │
│  │      0   4   8  12  16  20  ticks  │  │   0 ┤                 │ │
│  │  [On][Sus][Loop]  Sus:[3] L:[2][5] │  │  [On][Sus][Loop]      │ │
│  └────────────────────────────────────┘  └─────────────────────  ┘ │
│                                                                    │
│  Fadeout:[0200]  Vibrato: Type[0] Sweep[00] Depth[00] Rate[00]    │
│                                                                    │
│  Note → Sample mapping:                                            │
│  C-0·C#0·D-0·…  [00][00][01][00][01][01][00][…]                   │
└────────────────────────────────────────────────────────────────────┘
```

- Volume and panning **envelope graphs**: editable breakpoint curves.
  - Background `#000000`; grid lines `#202020`; curve `#80FF80` (vol) /
    `#80E0FF` (pan).
  - **Breakpoints**: `#FFFFFF` 3×3 px squares, draggable.
  - **Sustain point**: `#FFFF00` vertical dashed line.
  - **Loop region**: `#103060` overlay between loop-start and loop-end points.
- **[On]** / **[Sus]** / **[Loop]** — toggle buttons enabling envelope,
  sustain, and loop respectively.
- **Note → Sample map**: 96-key grid (C-0 … B-7), each cell shows the
  sample index assigned to that note.  Click to change.
- **Fadeout**: hex value 0000–FFFF; volume decrement per tick after note-off.
- **Vibrato**: auto-vibrato applied to all samples in this instrument.

---

## 12. Implementation Notes for egui

### Pixel-exact rendering
Use `egui::Painter` directly for pattern cells; do **not** use standard
`egui::Label` widgets which apply padding and anti-aliasing.

```rust
// Example: paint one pattern cell
painter.rect_filled(cell_rect, 0.0, Color32::from_rgb(96, 32, 64)); // cursor bg
painter.text(pos, Align2::LEFT_TOP, "C-4", font_id, Color32::WHITE);
```

### Font
The IBM EGA 8×8 font is already vendored and wired up.  Use it via:

```rust
use egui::{FontFamily, FontId};

let font_id = FontId::new(8.0, FontFamily::Name("tracker".into()));
```

`install_fonts()` in `crates/tracker-client/src/app.rs` registers the font
under the `"tracker"` family name (constant `FONT_TRACKER`) and sets it as
the default for both Proportional and Monospace families, so standard egui
widgets pick it up automatically.  Call `install_fonts(&cc.egui_ctx)` once
in `TrackerApp::new`; it is already called there.

### Pattern grid scroll
Use `egui::ScrollArea::both()` wrapping the cell grid; disable egui's
default scroll inertia for the snap-to-row feel.

### Channel columns
Each channel column is a fixed-width `ui.allocate_rect` block of
`87 px` (at 8-px font).  Rendering is entirely manual via `Painter`.

### Beat-row highlighting
Pre-compute which rows are primary / secondary beat rows from
`(row % highlight_secondary == 0)` and `(row % highlight_primary == 0)`
before the paint loop.

### Cursor blink
Use `ui.ctx().request_repaint_after(Duration::from_millis(500))` for a
500 ms blink cycle.  Track blink state in `TrackerApp`.

### Channel scopes
Sample the live output buffer (a ring buffer written by `FillCallback`)
to render per-channel waveforms via `painter.line_segment`.

---

## 13. Future: Divergences from MilkyTracker

While the initial implementation imitates MilkyTracker closely, the
following UX improvements are planned on top of the familiar base:

- **Keyboard shortcut overlay**: visible cheat-sheet panel (toggle with `?`)
  so users never need to memorise commands — the primary UX goal.
- **Command palette**: `Ctrl+P` fuzzy-search over all actions.
- **Undo/redo** via a proper history stack (MilkyTracker has limited undo).
- **Responsive layout**: the panel split ratios adjust to window width
  (important for WASM/browser use at arbitrary viewport sizes).
- **Accessible colour themes**: at least one high-contrast alternative to the
  classic dark palette.
