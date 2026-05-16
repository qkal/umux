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

The painted view should:

- paint the terminal background for the whole content area
- paint non-default cell backgrounds as horizontal runs where possible
- paint selection backgrounds over selected cells
- paint the cursor using the existing cursor color behavior
- paint text row-by-row or run-by-run using Floem text layout APIs
- use `TerminalCell` foreground, background, and style fields as the source of truth
- use `TerminalRendererSnapshot::version` as the primary content invalidation signal
- request repaint when snapshot version, grid size, cursor, or selection changes

The first pass does not need to implement font measurement, ligatures, advanced shaping, scrollback viewporting, terminal images, GPU-specific batching, or terminal protocol upgrades.

## Data Flow

The high-level data flow remains:

```text
ConPTY / shell output
  -> umux-terminal emulator
  -> TerminalRendererSnapshot
  -> TerminalUiState channel
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

This keeps the terminal behavior familiar while removing the large per-cell view churn from the display path.

## Reliability Requirements

The implementation should be incremental and reversible:

- keep the old per-cell renderer callable as a fallback
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
- background run grouping for repeated colors
- selection background decisions
- cursor foreground/background decisions
- zero-column and short-cell snapshots
- resize math remaining unchanged
- state coalescing continuing to keep the newest terminal snapshot

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
- a noisier output command remains responsive
- typing still echoes promptly enough
- drag selection and copy still work
- paste still writes to the terminal
- resizing the terminal still sends sane grid dimensions
- tabs, panes, workspace switching, and restore still work

Required checks before implementation completion:

- `cargo fmt --check`
- relevant `cargo test` targets for `umux-ui`, `umux-terminal`, and affected app/controller crates
- `cargo test --all` if the change touches shared behavior or if targeted tests are not enough
- manual launch smoke

## Acceptance Criteria

The repair is complete when:

- command output stutter is noticeably reduced compared with the current renderer
- typing echo is not worse and ideally feels more immediate
- normal terminal rendering no longer creates a child Floem view for every visible cell
- cursor, selection, foreground colors, background colors, and failure text render correctly
- terminal input, paste, copy, resize, notifications, tabs, panes, workspace switching, and restore remain functional
- the fallback renderer is still available during rollout
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
2. Add terminal draw-frame/run data structures and tests.
3. Add the custom Floem painted grid view.
4. Route normal terminal rendering through the painted grid while preserving input, pointer, resize, and clipboard handling at the existing boundary.
5. Run tests, launch smoke, and compare output burst behavior.

## Spec Self-Review

- Placeholder scan: no placeholder markers remain.
- Internal consistency: the design consistently targets the UI renderer while preserving terminal engine and app workflow behavior.
- Scope check: the work is focused enough for one implementation plan because engine rewrites, app redesign, browser surfaces, IPC, and terminal parity upgrades are explicitly out of scope.
- Ambiguity check: renderer boundary, fallback requirement, testing expectations, and acceptance criteria are explicit enough to plan implementation without guessing.
