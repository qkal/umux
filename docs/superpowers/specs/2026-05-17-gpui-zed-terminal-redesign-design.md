# GPUI Zed Terminal Redesign Design

Date: 2026-05-17

## Goal

Move `umux` fully from Floem to GPUI and redesign the app as a Zed-inspired, terminal-first workspace.

The migration should not be a thin renderer swap. GPUI should become the new product architecture: a workspace entity tree, explicit actions and focus contexts, compact pane and tab composition, and a terminal experience that strongly clones Zed where it helps. The app should keep its Windows-first `umux` identity and preserve the existing domain crates where they already provide good boundaries.

## Current Context

`umux` is a Rust workspace. The current desktop app is launched from `apps/umux` and delegates to `crates/umux-ui`.

The current UI is Floem-based:

```text
apps/umux
   |
   v
crates/umux-ui
   |-- shell.rs             top bar, workspace sidebar, panes, tabs, shortcuts
   |-- terminal_view.rs     terminal view, input, selection, refresh loop
   |-- terminal_canvas.rs   custom Floem terminal painter
```

The durable application logic is already separated:

```text
crates/umux-core       app model: windows, workspaces, panes, surfaces
crates/umux-app        AppController, TerminalRegistry, SessionStore
crates/umux-terminal   PTY, emulator, snapshots, input routing, notifications
```

This separation makes a GPUI migration feasible without rewriting the whole app at once. The expensive and brittle part is the UI layer: Floem signals, `dyn_stack` view composition, event bubbling, and terminal-specific view code are interleaved in `crates/umux-ui`.

The local `zed/` checkout is available as a read-only reference. Its GPUI runtime and terminal design are the primary inspiration sources.

## Approved Approach

Vendor GPUI runtime/platform crates and build a small `umux` UI kit inspired by Zed. Then replace the Floem runtime shell with a GPUI terminal-first workspace.

```text
vendor/gpui/*
vendor/umux-ui-kit/*
        |
        v
crates/umux-ui  -> terminal-first GPUI workspace
        |
        v
umux-app / umux-core / umux-terminal
```

The chosen approach is intentionally between two extremes:

- Do not depend on the local `zed/` checkout by path at runtime.
- Do not copy Zed's full workspace/editor/project stack wholesale.
- Do vendor the GPUI runtime and platform crates needed for `umux`.
- Do create a small Zed-inspired component layer for terminal tabs, icon buttons, popovers, keybinding labels, and theme tokens.
- Do strongly clone Zed terminal behavior and structure where it improves `umux`.
- Do selectively copy or adapt Zed terminal backend pieces when they materially improve correctness or user experience.

## Rejected Approaches

### Published GPUI Only

Using only the published GPUI crate gives a cleaner dependency story, but GPUI is pre-1.0 and Zed often carries the freshest behavior in-tree. The user explicitly prefers vendoring so the migration can move in lockstep with the local Zed reference and keep a stable local foundation.

### GPUI Runtime Only

Vendoring only GPUI and building every UI primitive from scratch minimizes copied code, but it slows down the path to a Zed-like experience. `umux` needs tabs, buttons, icon buttons, popovers, keybinding hints, and theme tokens immediately. A small `umux-ui-kit` is the right middle ground.

### Full Zed Workspace Transplant

Copying Zed's workspace, pane, terminal view, editor, project, task, and settings stack would create a large dependency graph and force terminal-first `umux` into an editor-shaped architecture. Zed should guide the design, but `umux` should keep its own compact terminal workspace.

## Target Architecture

`crates/umux-ui` should become a GPUI app shell.

```text
apps/umux
   |
   v
crates/umux-ui
   |
   |-- runtime.rs
   |     gpui launch, window options, startup wiring
   |
   |-- workspace.rs
   |     UmuxWorkspace entity
   |
   |-- actions.rs
   |     GPUI actions -> AppAction / terminal commands
   |
   |-- shell/
   |     |-- top_bar.rs
   |     |-- workspace_rail.rs
   |     |-- pane_group.rs
   |     |-- surface_tabs.rs
   |
   |-- terminal/
         |-- terminal_surface.rs
         |-- terminal_element.rs
         |-- terminal_bridge.rs
```

`crates/umux-ui-kit` should provide the small reusable layer that makes the app feel coherent without pulling in Zed's whole UI crate.

```text
crates/umux-ui-kit
   |-- theme.rs
   |-- button.rs
   |-- icon_button.rs
   |-- tab.rs
   |-- tab_bar.rs
   |-- popover.rs
   |-- keybinding.rs
```

The runtime vendoring should be explicit and auditable.

```text
vendor/
  gpui/
    gpui/
    gpui_platform/
    gpui_windows/
    gpui_wgpu/
    gpui_macros/
    gpui_shared_string/
    gpui_util/
```

Add only the GPUI crates required for the Windows build, plus any small support crates proven necessary by compilation. Zed UI/workspace/editor/project crates should remain reference material unless a later design explicitly approves copying a narrow piece.

## Zed Inspiration Map

```text
Zed idea                         umux design
-----------------------------    -----------------------------
Application + App                GPUI launch and app globals
Workspace entity                 UmuxWorkspace entity
Pane / PaneGroup                 Terminal pane group
Item tabs                        Surface tabs
Actions and KeyContext           GPUI actions for workspace and terminal
TerminalView entity              TerminalSurface entity
TerminalElement custom element   UmuxTerminalElement
Dock / Panel                     later notifications/browser/settings areas
Editor/project systems           not copied into the first migration
```

The goal is to clone the feel and useful architecture, not the entire application.

## Interaction Model

The app should feel like a Zed-style workspace optimized for live terminals.

```text
+-------------------------------------------------------------+
| umux        workspace: umux        [jump] [new] [command]   |
+----------+--------------------------------------------------+
| alpha  * |  tab: powershell      tab: cargo test      +     |
| beta     +--------------------------------------------------+
| server * |                                                  |
|          |  terminal pane                                   |
| + ws     |                                                  |
|          |                                                  |
|          +--------------------------+-----------------------+
|          |  terminal pane           |  terminal pane        |
|          |                          |                       |
+----------+--------------------------+-----------------------+
```

Primary rules:

- The left workspace rail remains visible by default.
- Each workspace is a terminal workspace rooted at a cwd.
- Unread and build-finished state rolls up to the workspace row.
- The center area is a split pane grid.
- Pane focus is visible and keyboard-driven.
- Splits are terminal-oriented: split right/down creates a live terminal.
- Terminal tabs sit at the top of each pane.
- Tabs are compact, closeable, renameable, and show status/unread state.
- Browser, settings, and notifications are not center-stage in milestone one.
- Keyboard actions are the primary interaction path.

The first milestone may preserve today's one-level split limit if expanding the model would slow down the migration. Multi-level arbitrary pane trees can be designed later.

## Keyboard And Focus

GPUI actions should replace ad hoc Floem `KeyDown` routing.

```text
Ctrl+N             new workspace
Ctrl+T             new terminal tab
Ctrl+W             close terminal tab
Ctrl+Shift+W       close workspace
Ctrl+Alt+D         split right
Ctrl+Shift+Alt+D   split down
Ctrl+1..8          jump workspace
Ctrl+9             jump last workspace
Ctrl+Shift+U       jump latest unread
Ctrl+Shift+C/V     terminal copy/paste
```

Focus should be explicit:

```text
workspace rail <-> pane group <-> active terminal
                         |
                         v
                  terminal input mode
```

The workspace owns app-level actions. Panes own pane actions. Terminals own terminal actions. This mirrors Zed's focus-aware action model and avoids brittle event bubbling.

## Data Flow

Durable app state should stay under the existing controller.

```text
User input
   |
   v
GPUI action / terminal element event
   |
   v
UmuxWorkspace entity
   |
   v
AppController::apply(AppAction)
   |
   +--> AppModel changes
   +--> TerminalRegistry spawns/closes sessions
   +--> SessionStore saves model
   |
   v
cx.notify()
   |
   v
Render updated workspace shell
```

Terminal output should flow through a GPUI bridge:

```text
ConPTY / shell output
   |
   v
umux-terminal emulator or Zed-derived terminal backend
   |
   v
TerminalRendererSnapshot / richer terminal content
   |
   v
TerminalBridge channel/coalescer
   |
   v
TerminalSurface entity state
   |
   v
UmuxTerminalElement paint
```

Terminal input should keep a single routing boundary:

```text
GPUI KeyDownEvent
   |
   v
TerminalInputRouter or Zed-derived key mapping
   |
   v
WriteBytes / CopySelection / PasteClipboard / Ignore
   |
   v
TerminalEntry::send_input(...)
```

`AppController` remains the single mutating authority for workspace, pane, and surface changes. GPUI entities should not mutate `AppModel` directly except through controller methods or intentionally added controller-adjacent helpers.

Session saves should remain immediate during the first migration. Throttled persistence can be added later after the runtime migration is stable.

## State Ownership

```text
UmuxWorkspace
  owns:
    AppController
    SessionStore
    startup warning
    selected/focused pane metadata
    subscriptions to terminal notifications

TerminalSurface
  owns:
    surface_id
    latest terminal content
    health/status
    selection
    drag state
    scroll state
    context menu state
    rename state
    bridge handle

UmuxTerminalElement
  owns:
    prepared draw frame
    hitboxes
    mouse mapping
    paint and low-level input hooks
```

## Terminal Experience

The terminal target is stronger than "inspired by Zed." The goal is to clone the Zed terminal experience where it is relevant to a terminal-first `umux`.

```text
Zed terminal shape                    umux clone target
----------------------------------    ----------------------------------
TerminalPanel with pane group         terminal-first center pane group
TerminalView entity                   TerminalSurface entity
TerminalElement custom painter        UmuxTerminalElement custom painter
Tab content with icon/status/title    compact terminal tabs with status
Right-click context menu              copy/paste/clear/rename menu
Focus-aware key context               terminal/workspace action contexts
Scrollbar + scrollback navigation     GPUI terminal scrollbar
Rename editor in tab                  inline terminal tab rename
Task/status icons                     process/status/unread indicators
```

First milestone terminal behavior should include:

- dense terminal tab bar
- terminal icon and status in tab
- inline terminal rename
- right-click context menu
- focus-aware copy/paste
- selection and word selection
- scrollback scrollbar
- split/new terminal controls in the tab bar
- custom GPUI terminal element with batched painting

Later milestones can add terminal search, task/rerun status, breadcrumbs, hyperlink hover/tooltips, richer settings, and deeper backend convergence.

## Terminal Rendering

The GPUI terminal renderer should follow Zed's `TerminalView` / `TerminalElement` split.

```text
TerminalSurface entity
   |
   | prepares and owns live state:
   |   - content/snapshot
   |   - health/status
   |   - selection
   |   - scroll offset
   |   - terminal bounds
   |   - focus
   v
UmuxTerminalElement
   |
   | low-level GPUI element:
   |   - layout
   |   - paint
   |   - mouse hit mapping
   |   - key event handoff
   v
GPU-rendered terminal grid
```

The current pure draw preparation from `terminal_canvas.rs` can seed the first GPUI implementation. It should evolve toward Zed's richer layout model:

```text
Terminal content
   |
   v
prepare layout state
   |
   +--> batched text runs
   +--> background rects
   +--> selection rects
   +--> cursor layout
   +--> hyperlink/hover ranges later
   |
   v
GPUI paint calls
```

The first pass should preserve:

- terminal typing
- terminal resize into cols/rows
- copy and paste
- drag selection
- unread notification propagation
- terminal failure snapshots
- status/title display

The first pass should improve:

- lower view count
- no Floem runtime path
- focus-aware terminal input
- clearer render boundary
- Zed-like tab and terminal affordances

## Terminal Backend Strategy

`umux-terminal` should remain the compatibility shell at first, but Zed terminal backend code should be copied or adapted when it improves the terminal.

Strong candidates from `zed/crates/terminal` include:

- key mappings
- mouse mappings
- color mappings
- `TerminalBounds`
- scrollback and display offset behavior
- selection phases
- mouse reporting
- hyperlink/path detection
- terminal settings structure
- event coalescing model

The migration should not copy the full Zed terminal crate on day one. Zed's terminal backend depends on GPUI, task, settings, theme, project-adjacent concepts, and workspace expectations. Copying it wholesale before the GPUI shell exists would obscure the migration and increase risk.

Use this rule:

```text
Copy from Zed when it improves:
  terminal correctness
  input and mouse behavior
  scrollback/search/hyperlinks
  render state richness
  GPUI integration

Keep or rewrite in umux when Zed code depends heavily on:
  project crate
  task system
  editor crate
  settings/theme globals
  workspace item machinery
```

The long-term target is a Zed-like terminal backend, not merely a Zed-like view. The short-term path is controlled grafting with tests.

## Milestone 1: GPUI Terminal Workspace

Goal: `umux` launches on GPUI and is usable as a terminal-first workspace.

Includes:

- vendored GPUI runtime/platform crates
- small `umux-ui-kit` with theme, buttons, tabs, icon buttons
- GPUI app launch replacing Floem launch
- `UmuxWorkspace` entity owning `AppController` and `SessionStore`
- workspace rail
- pane group
- terminal tabs
- new/close workspace
- new/close terminal tab
- split right/down, preserving current split limits if needed
- jump latest unread
- GPUI `TerminalSurface` and `UmuxTerminalElement`
- terminal typing, resize, selection, copy, paste
- unread notification propagation
- session restore/save

Not included:

- browser webview
- command palette
- full Zed terminal backend replacement
- terminal search
- hyperlink opening
- task runner integration
- arbitrary multi-level pane trees unless it falls out cheaply

## Milestone 2: Zed Terminal Feel

Goal: terminal behavior starts matching Zed more closely.

Includes:

- Zed-style terminal tab content: icon, status, title, unread
- inline terminal rename
- right-click context menu
- scrollback scrollbar
- word selection and better mouse handling
- copied/adapted Zed key, mouse, and color mappings where useful
- terminal settings shell: font, cursor, scrollback

## Milestone 3: Zed Terminal Power

Goal: bring over deeper terminal affordances.

Includes:

- terminal search
- hyperlink/path detection
- hover tooltips
- richer scrollback behavior
- clear scrollback action
- terminal status/task-like indicators
- evaluation of replacing more of `umux-terminal` with Zed-derived backend code

## Milestone 4: Workspace Polish

Goal: make the surrounding workspace feel coherent and fast.

Includes:

- command palette
- better focus navigation
- dock/panel-inspired notifications
- browser surface plan
- settings UI
- persistence throttling

## Error Handling

Startup should preserve the current session-load warning behavior. If a saved session cannot be read or restored, the GPUI shell should show a compact warning banner and open a fresh workspace.

Terminal spawn failures should render as visible terminal-like failure content. A failed terminal should remain a tab with a clear status instead of silently disappearing.

Action dispatch failures should be logged and should not leave GPUI state diverged from `AppController`.

Terminal bridge failures should mark the terminal surface failed and keep the tab visible.

## Testing Strategy

Automated tests should cover:

- `AppController` behavior remains stable
- session restore/save survives the GPUI migration
- shortcut/action mapping
- GPUI action dispatch into `AppAction`
- terminal draw-frame preparation
- terminal bounds and resize math
- copied Zed key/mouse/color mappings adapted correctly
- terminal selection/copy/paste routing
- unread notification propagation
- terminal bridge coalescing
- failed terminal rendering state

Manual smoke should include:

- `cargo run -p umux` launches the GPUI app
- terminal accepts input
- noisy output remains responsive
- split panes work
- tabs open and close
- terminal tab rename works
- workspace rail selection works
- context menu copy/paste works
- notifications mark unread and jump latest unread works
- session restores after restart

## Risk Controls

- Keep `umux-core`, `umux-app`, and `umux-terminal` stable while replacing the UI shell.
- Vendor GPUI in an explicit directory and document copied source provenance.
- Copy Zed terminal backend pieces in small, tested slices.
- Avoid importing Zed workspace, editor, and project crates.
- Keep the old Floem code available as a temporary branch/reference during implementation, but do not keep Floem as a permanent runtime path.
- Build after each meaningful migration slice.

## Open Follow-Ups

These are intentionally deferred beyond this design:

- exact vendored GPUI crate list after compilation proves dependencies
- file-level implementation plan
- whether arbitrary nested split trees should be part of milestone one
- exact provenance format for copied Zed code
- terminal settings schema
- command palette design
