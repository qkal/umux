# Terminal Renderer Repair Design

Date: 2026-05-16

## Goal

Repair the terminal's clumsy output behavior by replacing the expensive per-cell Floem view renderer with a dedicated low-view-count terminal painter.

The main user-visible problem is command output stutter. Typing echo also feels slightly delayed, but it is secondary. The repair should target the rendering bottleneck first while preserving the current terminal engine, app workflow, session ownership, tabs, panes, notifications, selection, clipboard behavior, and restore model.

## Current Context

`crates/umux-terminal` already owns terminal behavior: ConPTY integration, live sessions, Alacritty-based emulation, snapshots, input routing, resize, health state, and OSC notification extraction.

`crates/umux-ui/src/terminal_view.rs` currently renders each visible terminal cell as its own Floem view. Every snapshot is expanded into rows, cells, labels, background containers, and long string keys that include snapshot version, position, character, colors, cursor, and selection state. That creates a large amount of view diffing and allocation during output bursts.

Floem 0.2.0 supports custom `View` implementations with direct painting through `PaintCx`, including shape fills and text drawing. That is the right layer for a durable terminal renderer.

## Approved Approach

Build a dedicated painted terminal grid view in `umux-ui`.

The new renderer should consume `TerminalRendererSnapshot` and paint the visible terminal grid directly with a small, stable number of Floem views. It should draw background runs, text runs, cursor, and selection in the custom view's paint pass.

The existing per-cell renderer should remain available as a fallback during rollout. Keeping the fallback reduces implementation risk and makes it easier to compare behavior while the painted renderer is verified.

The rollout switch should be concrete and observable. Use `UMUX_TERMINAL_RENDERER=painted|legacy`, defaulting to `painted` after the new renderer is wired. The legacy value must force the old per-cell path so bug reports can quickly separate painter issues from terminal engine issues.

## Rejected Approaches

### Refresh Loop Tuning Only

Changing the 33 ms refresh cadence or coalescing more aggressively might reduce some redraw pressure, but it leaves the core cost intact: thousands of child views and expensive keys for normal terminal output. This is not enough for the reported output stutter.

### Row Virtualization

Virtualized rows are useful for very large lists, but this terminal renderer already draws only visible rows. The expensive part is the per-cell view tree inside those rows, so virtualization is an incomplete fix.

### Terminal Engine Rewrite

The symptom points at the UI renderer, not ConPTY or the emulator. Rewriting the terminal engine would create a much larger blast radius without addressing the observed rendering bottleneck directly.

## Renderer Boundary

The repair should stay inside `crates/umux-ui`, either in `terminal_view.rs` or in a small sibling module such as `terminal_canvas.rs`.

The renderer boundary should expose a clear input:

```text
TerminalRendererSnapshot
TerminalSelection
TerminalMetrics
```

and produce a painted terminal grid view.

Internally, the boundary should split into two parts:

1. a pure frame preparation layer that converts a snapshot, selection, and metrics into draw commands
2. a Floem custom view that stores the latest prepared frame and paints it

The custom view must follow Floem's explicit state-update model. Reactive effects should observe the terminal UI state and selection signals, call `ViewId::update_state` with the latest prepared frame, and the view's `update` method should request paint or layout as appropriate. The view must not rely on reading reactive signals directly inside `paint`.

The following behavior should remain owned by the existing code:

- terminal session startup and shutdown
- `TerminalInputRouter` key routing
- paste and copy behavior
- pointer-to-cell selection math
- terminal resize math
- notification drain and unread propagation
- tab, pane, workspace, restore, and app controller behavior

## Rendering Behavior

The first implementation should preserve the current fixed `TerminalMetrics` values: 8 px cell width and 16 px cell height.

The painted grid should fill the available terminal viewport owned by the existing terminal view. Its layout must not shrink to only the current `cols * cell_width` by `rows * cell_height` content size, because the resize handler depends on the viewport size to compute terminal grid dimensions. Terminal resize math should continue to run at the same boundary that currently receives the grid view's resize events.

The painted view should:

- paint the terminal background for the whole content area
- paint non-default cell backgrounds as horizontal runs where possible
- paint selection backgrounds over selected cells
- paint the cursor using the existing cursor color behavior
- paint text as fixed-cell runs using Floem text layout APIs
- use `TerminalCell` foreground, background, and supported style fields as the source of truth
- use `TerminalRendererSnapshot::version` as the primary content invalidation signal
- request repaint when snapshot version, grid size, cursor, or selection changes

Text placement should preserve terminal grid geometry. Every text run should carry its row and starting column. Paint each run at `x = start_col * cell_width` and `y = row * cell_height` with a stable baseline offset, rather than letting a full-row text layout decide all column positions. This prevents proportional glyph advance, fallback font metrics, or layout rounding from drifting away from terminal cell coordinates.

The first pass should preserve current behavior for style fields that the legacy renderer currently ignores. If `bold`, `italic`, `underline`, or `inverse` are implemented in the painted renderer, add tests for them. If they are not implemented in the first pass, document that as legacy parity and ensure the fallback remains available. The painter must still consume foreground and background colors, cursor state, and selection state correctly.

The first pass does not need to implement font measurement, ligatures, advanced shaping, scrollback viewporting, terminal images, GPU-specific batching, or terminal protocol upgrades. Wide glyphs, combining marks, and non-ASCII glyphs should not be treated as solved by the first pass unless explicitly tested. For rollout, the painter may preserve current one-cell-per-`char` snapshot behavior and use the legacy renderer as the escape hatch for any unsupported text shaping issue.

## Data Flow

The high-level data flow remains:

```text
ConPTY / shell output
  -> umux-terminal emulator
  -> TerminalRendererSnapshot
  -> TerminalUiState channel
  -> umux-ui signal
  -> prepared terminal draw frame
  -> Floem custom view state update
  -> painted umux-ui terminal grid
```

Input remains:

```text
Floem key event
  -> TerminalInputRouter
  -> LiveTerminalSession::send_input
  -> shell
  -> echoed output snapshot
  -> painted terminal grid repaint
```

Selection-only changes must also repaint the grid. Dragging a selection changes `TerminalSelection` without changing `TerminalRendererSnapshot::version`, so the draw-frame preparation key must include selection, cursor, grid size, and metrics in addition to the snapshot version.

This keeps the terminal behavior familiar while removing the large per-cell view churn from the display path.

## Reliability Requirements

The implementation should be incremental and reversible:

- keep the old per-cell renderer callable as a fallback
- route fallback selection through `UMUX_TERMINAL_RENDERER=painted|legacy`
- keep terminal engine, app controller, model, session store, and terminal registry unchanged
- avoid behavior changes to terminal input, paste, copy, selection, resize, notifications, panes, tabs, and restore
- keep terminal spawn failures rendering through a snapshot-like visible terminal frame
- build after each meaningful slice

If the custom painter cannot support a behavior cleanly in the first pass, the behavior should stay on the fallback path until it can be implemented safely.

## Error Handling

Terminal spawn failures should continue to produce visible terminal text with a `terminal failed` status. The painted renderer should display that failure snapshot like ordinary terminal content.

Renderer construction failure should not take down app startup. During rollout, if the painted renderer is disabled or unavailable, the UI should be able to use the fallback renderer.

## Testing Strategy

Automated tests should cover:

- snapshot-to-draw-run conversion
- text run grouping across simple rows
- fixed-cell text run coordinates, including non-zero starting columns
- background run grouping for repeated colors
- selection background decisions
- cursor foreground/background decisions
- selection-only repaint/frame changes when snapshot version is unchanged
- draw-frame invalidation when grid size, cursor, or metrics change
- legacy parity for bold, italic, underline, and inverse if they remain unimplemented
- zero-column and short-cell snapshots
- resize math remaining unchanged
- state coalescing continuing to keep the newest terminal snapshot
- renderer mode parsing for painted and legacy fallback values

Existing relevant tests should continue to pass for:

- terminal input routing
- terminal snapshots
- selection text extraction
- notification/unread propagation
- app controller and terminal registry behavior

Manual smoke should include:

- `cargo run -p umux` launches the app
- `dir` renders readable output without obvious stutter
- `cargo --version` renders promptly
- a repeatable noisy output command remains responsive, for example a loop that prints at least 1,000 lines
- typing still echoes promptly enough
- drag selection and copy still work
- paste still writes to the terminal
- resizing the terminal still sends sane grid dimensions
- tabs, panes, workspace switching, and restore still work
- `$env:UMUX_TERMINAL_RENDERER='legacy'; cargo run -p umux` launches with the old renderer on Windows PowerShell

Required checks before implementation completion:

- `cargo fmt --check`
- relevant `cargo test` targets for `umux-ui`, `umux-terminal`, and affected app/controller crates
- `cargo test --all` if the change touches shared behavior or if targeted tests are not enough
- manual launch smoke

## Acceptance Criteria

The repair is complete when:

- command output stutter is noticeably reduced compared with the current renderer on the manual noisy-output smoke
- typing echo is not worse and ideally feels more immediate
- normal terminal rendering no longer creates a child Floem view for every visible cell; the normal path uses one painted grid view plus the surrounding terminal chrome
- cursor, selection, foreground colors, background colors, and failure text render correctly
- terminal input, paste, copy, resize, notifications, tabs, panes, workspace switching, and restore remain functional
- the fallback renderer is still available during rollout through `UMUX_TERMINAL_RENDERER=legacy`
- automated checks and a manual launch smoke pass

## Out Of Scope

- rewriting ConPTY or `alacritty_terminal`
- replacing the terminal emulator
- terminal protocol parity work
- dynamic font measurement
- ligatures and advanced shaping
- scrollback viewporting
- browser surfaces
- CLI/IPC transport
- app workflow redesign
- session schema changes

## Implementation Notes

The implementation plan should start by extracting the existing cell renderer behind a named fallback function. Then add a small testable frame/run preparation layer before introducing the custom painted view.

Suggested implementation slices:

1. Extract and preserve the existing per-cell renderer as fallback.
2. Add renderer mode parsing for `UMUX_TERMINAL_RENDERER=painted|legacy`.
3. Add terminal draw-frame/run data structures and tests.
4. Add the custom Floem painted grid view with explicit `ViewId::update_state` reactivity.
5. Route normal terminal rendering through the painted grid while preserving input, pointer, resize, and clipboard handling at the existing boundary.
6. Run tests, launch smoke in both renderer modes, and compare output burst behavior.

## Spec Self-Review

- Placeholder scan: no placeholder markers remain.
- Internal consistency: the design consistently targets the UI renderer while preserving terminal engine and app workflow behavior.
- Scope check: the work is focused enough for one implementation plan because engine rewrites, app redesign, browser surfaces, IPC, and terminal parity upgrades are explicitly out of scope.
- Ambiguity check: renderer boundary, fallback requirement, Floem reactivity, viewport layout, text coordinates, testing expectations, and acceptance criteria are explicit enough to plan implementation without guessing.
