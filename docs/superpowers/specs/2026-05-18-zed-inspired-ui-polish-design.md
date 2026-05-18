# Zed-Inspired UI Polish Design

Date: 2026-05-18

## Goal

Make the current GPUI `umux` interface feel more modern, cohesive, and Zed-inspired without changing the app model or starting a broader feature rebuild.

The app already has the right high-level shape: a top bar, workspace rail, split pane area, and terminal tabs. The problem is that the current chrome still feels skeletal. This pass should make it feel like a real terminal workspace: calm, dense, readable, and explicit about focus.

## Current Context

`umux` is a Windows-first Rust workspace. The desktop app runs through `apps/umux` and `crates/umux-ui`, using vendored GPUI crates.

The current UI implementation is already split into small GPUI shell modules:

```text
crates/umux-ui
  workspace.rs
  shell/
    top_bar.rs
    workspace_rail.rs
    pane_group.rs
    surface_tabs.rs
  terminal/
    terminal_surface.rs
    terminal_element.rs

crates/umux-ui-kit
  theme.rs
  button.rs
  icon_button.rs
  keybinding.rs
  tab.rs
```

There is also an approved prior design, `docs/superpowers/specs/2026-05-17-gpui-zed-terminal-redesign-design.md`, that moved the app toward a GPUI and Zed-inspired architecture. This design is a narrower follow-up that polishes the shell now that the GPUI implementation exists.

## Approved Approach

Do a focused polish pass with a small interaction clarity slice.

```text
+------------------------------------------------------------------+
| umux  workspace: umux                            +  |  split  | ! |
+------------+-----------------------------------------------------+
| WORKSPACES |  terminal        cargo test *        server      +   |
| > umux     +-----------------------------------------------------+
|   docs     |                                                     |
|   api  *   |  active terminal pane                               |
|            |  stronger focus border, darker editor surface       |
| +          |                                                     |
+------------+--------------------------+--------------------------+
|            | terminal                 | terminal                 |
|            | inactive pane            | inactive pane            |
+------------+--------------------------+--------------------------+
```

This keeps the same product shape and data flow, but improves the visible system:

- top bar becomes intentional workspace chrome
- rail rows look selected, clickable, and scannable
- pane focus is visually obvious
- tabs become compact Zed-like segments
- controls stop looking like temporary text placeholders
- unread state is shown once, consistently
- warning banners look like status strips instead of accidental extra rows

## Rejected Approaches

### Full Workspace Chrome Upgrade

Adding a richer status area, command affordances, and context panels would make the app feel more complete, but several of those features are not fully wired. This pass should avoid designing around unavailable behavior.

### Deep Interaction Repair

Reworking focus ownership, key routing, and terminal refresh behavior could address deeper perceived flakiness, but it is a larger behavioral change. This pass should only improve interaction clarity where it fits naturally into the shell polish.

### Full Zed UI Transplant

Copying Zed's UI components or icon stack wholesale would increase dependency and provenance surface for a polish pass. `umux` should remain compact and use Zed as a visual guide rather than a source transplant here.

## Design

### Theme

Update `umux-ui-kit` theme tokens toward a neutral Zed One Dark style:

```text
BACKGROUND      app/window background
SURFACE         editor or terminal workspace surface
PANEL           rail and top bar surfaces
ELEVATED        selected tab and active controls
BORDER          normal dividers
BORDER_STRONG   active pane and focused input borders
TEXT            primary text
MUTED_TEXT      secondary text
DIM_TEXT        tertiary text
ACCENT          focus/unread/action accent
WARNING         warning strip color
WARNING_TEXT    warning strip text
HOVER           hoverable row/control fill
ACTIVE          active/pressed row/control fill
```

Keep the existing public exports stable where practical, especially `BACKGROUND`, `PANEL`, `BORDER`, `TEXT`, `MUTED_TEXT`, and `UNREAD_BLUE`.

### Top Bar

The top bar should read as workspace chrome, not a placeholder title row.

```text
+------------------------------------------------------------------+
| umux  workspace: backend                          +  split  bell |
+------------------------------------------------------------------+
```

Required changes:

- keep brand on the left
- show current workspace as contextual text
- add compact clickable action affordances for new tab, split right, split down, and jump latest unread
- use muted labels and subtle borders so controls feel integrated
- wire those controls through existing `UmuxWorkspace` callbacks and `AppAction` paths, not through new model behavior
- use cursor and hover affordances so clickable controls do not feel inert

Expected callback shape:

```text
top_bar(
  title,
  on_new_terminal_tab,
  on_split_right,
  on_split_down,
  on_jump_latest_unread,
)
```

These actions already exist as GPUI actions and controller dispatches, so the top bar should expose them rather than rendering dead chrome.

### Workspace Rail

The rail should be compact and scannable.

```text
+------------+
| WORKSPACES |
| > backend  |
|   docs     |
|   api    * |
|            |
|     +      |
+------------+
```

Required changes:

- add a small section label
- make selected row clearly selected with active fill and accent mark
- keep unread badges visually distinct as a separate element from the label
- replace `+ ws` with a compact add control
- make text truncation predictable
- remove duplicate unread `*` from labels if a visual badge is already shown
- use stable row height and badge dimensions so unread and selected states do not resize the rail
- use cursor and hover affordances on selectable rows and add controls

### Pane Group

Panes should make focus obvious without visual noise.

```text
+---------------------------+---------------------------+
| active pane               | inactive pane             |
| accent edge + tab bar     | quiet border + tab bar    |
| terminal                  | terminal                  |
+---------------------------+---------------------------+
```

Required changes:

- active pane uses stronger border or accent edge
- inactive panes stay quiet but still separated
- pane backgrounds use the editor/terminal surface token
- split borders stay thin and consistent
- border widths remain stable across selected and unselected panes so focus changes do not shift layout

### Surface Tabs

Tabs should look like compact workspace tabs.

```text
+-----------------------------------------------------+
|  terminal  |  cargo test *  |  server logs   x  | + |
+-----------------------------------------------------+
```

Required changes:

- selected tab has active fill and clear text color
- inactive tabs use muted text and hover fill
- unread appears once as a separate compact marker element, not by mutating the title string
- close control is stable and does not shift tab layout unexpectedly
- rename control is compact and available only where it already is today
- inline rename editor has a stronger border and stable width
- tabs keep stable height, close-control width, and marker dimensions across selected, unread, and rename states
- clickable tab controls use cursor and hover affordances

Source files should remain ASCII unless there is a strong reason to use Unicode. ASCII-safe controls such as `+`, `x`, and short labels are acceptable for this pass.
If the unread marker is ASCII, it should still be rendered as its own child element, not appended to `label`.

### Warning Strip

Startup warnings should look intentional.

```text
+------------------------------------------------------------------+
| ! Previous session could not be restored. Opened a fresh workspace |
+------------------------------------------------------------------+
```

Required changes:

- use warning-specific theme tokens
- add a small marker at the left
- preserve the existing warning messages and startup flow

## Data Flow

No model, persistence, or controller changes are required.

```text
click / key
   -> shell component callback
   -> UmuxWorkspace weak entity update
   -> AppController::apply(AppAction)
   -> save model when required
   -> cx.notify()
```

The polish should work with the existing callbacks for:

- selecting workspaces
- creating workspaces
- selecting surfaces
- closing surfaces
- creating terminal tabs
- starting and applying tab rename edits
- app-level GPUI actions already registered in `UmuxWorkspace`

Top-bar buttons should reuse the same callback pattern already used by `workspace_rail` and `surface_tabs`: closures owned by `UmuxWorkspace`, passed into the shell component, and dispatched through `dispatch_from_weak` or `dispatch_actions_from_weak`.

## Files

Expected implementation scope:

```text
crates/umux-ui-kit/src/theme.rs
crates/umux-ui-kit/src/lib.rs
crates/umux-ui/src/shell/top_bar.rs
crates/umux-ui/src/shell/workspace_rail.rs
crates/umux-ui/src/shell/surface_tabs.rs
crates/umux-ui/src/shell/pane_group.rs
crates/umux-ui/src/workspace.rs
crates/umux-ui/src/view_model.rs
```

`view_model.rs` should stop adding unread suffixes to labels when the rendering components already receive `unread` booleans and draw a marker.
Existing `umux-ui-kit` tab helpers may remain for compatibility, but the refreshed shell must not use a helper that appends unread text and also render a marker.

## Testing And Verification

Required checks:

```text
cargo fmt --check
cargo test -p umux-ui-kit
cargo test -p umux-ui view_model
cargo test -p umux-ui shell
cargo check -p umux
```

If the build succeeds, launch the app locally with:

```text
cargo run -p umux
```

Manual verification should confirm:

- top bar, rail, tabs, warning strip, and panes use the refreshed theme coherently
- selected workspace and selected pane are obvious
- unread state appears once per row/tab
- top-bar controls for new tab, split right, split down, and latest unread are clickable
- close, add, and rename controls remain clickable
- inline rename still accepts text, backspace, enter, and escape
- terminal input still works after the visual changes

## Out Of Scope

- real command palette
- settings UI
- browser webview
- workspace rename UI
- full icon library integration
- terminal renderer or refresh-loop redesign
- session schema changes
- new persistence behavior

## Acceptance Criteria

The pass is complete when:

- the interface looks cohesive and visibly more modern
- the shell reads as a Zed-inspired terminal workspace
- selected workspace, selected tab, and selected pane are clearly distinct
- unread and warning states are visible without duplicate signaling
- clickable controls have hover/cursor feedback and do not render as inert decoration
- existing shell interactions still work
- the required Rust checks pass, or any failures are documented with concrete blockers
