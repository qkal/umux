# Release Hardening Design

Date: 2026-05-16

## Goal

Prepare `umux` for an MVP release by improving supportability and aligning visible behavior with the current product scope.

This first hardening pass deliberately avoids the larger IPC and WebView implementation work. It focuses on diagnostics, startup error visibility, and shortcut truthfulness so a terminal-first MVP is easier to debug and less likely to surprise users.

## Current Context

`umux` is a Windows-first Rust workspace with a terminal-first UI, app controller, session persistence, terminal registry, browser state model, CLI request builder, and IPC protocol types.

The latest renderer performance work has addressed the previous terminal renderer hot spot. The current pre-MVP gaps most suitable for a small release-hardening pass are:

- `tracing` exists in workspace dependencies but runtime logging is not initialized or used.
- session load failures are collapsed into a fresh seed model without visible user feedback.
- restored controller failures fall back to a seed model without visible user feedback.
- default shortcut declarations in `umux-config` describe more behavior than `umux-ui` currently handles.
- CLI/IPC and real browser WebView rendering remain larger independent subsystems and are not part of this slice.

## Approved Approach

Build a focused release-hardening slice with three outcomes:

1. initialize runtime diagnostics for the GUI process
2. make session startup/restore failures visible and logged
3. align runtime shortcuts with the MVP surface

The implementation should stay mostly in `crates/umux-ui`, with small model or controller additions only when needed to support existing declared shortcut actions.

## Diagnostics

Add a tiny diagnostics initializer around app startup. It should initialize `tracing_subscriber` once for the GUI process, default to useful `info`-level logging, and allow opt-in verbosity through existing Rust logging conventions.

Preferred behavior:

- respect `RUST_LOG` when set
- otherwise respect `UMUX_LOG` when set
- otherwise use a default filter suitable for MVP support, such as `umux=info,warn`
- do not panic if logging has already been initialized by tests or embedding code

Initial instrumentation should cover the startup and release-hardening paths touched by this slice:

- session file load result
- corrupt or unsupported session fallback
- I/O session load failure
- restored controller spawn failure
- session save failure after actions or terminal notifications
- unhandled shortcut actions that remain intentionally out of MVP scope

## Startup Error Visibility

Replace the current silent startup fallback with explicit classification.

Session startup should treat cases differently:

- missing session file: seed a normal fresh model without warning
- corrupt, invalid UTF-8, or unsupported schema: keep the existing rename-aside behavior and surface a warning that the previous session could not be restored
- transient I/O error: seed a fresh model but surface a warning that the session file could not be read
- restored controller failure: seed a fresh model but surface a warning that restored terminal sessions could not be started

The UI should show startup warnings in a compact non-blocking banner near the top of the shell. The banner is not a modal and should not prevent terminal use.

The banner text should be short and user-facing. Detailed error strings should go to logs.

Session save failures should at least be logged. If the warning plumbing is already straightforward, save failures may also update the same banner; otherwise they remain log-only in this slice to keep scope tight.

## Shortcut Alignment

The runtime shortcut mapper in `umux-ui` should stop silently drifting away from `umux-config::default_shortcuts`.

For this MVP slice:

- wire declared shortcuts whose actions already exist and are safe in the current terminal-first UI
- leave browser, address bar, settings, and notification-center shortcuts unwired until those surfaces exist
- leave terminal copy and paste owned by the terminal input router
- add explicit tests documenting which default shortcut actions are handled by the shell, terminal, or intentionally deferred

Safe shell-level candidates include:

- `new_workspace`
- `jump_workspace_1_8`
- `jump_last_workspace`
- `close_workspace`
- `new_surface`
- `close_surface`
- `split_right`
- `split_down`
- `jump_latest_unread`

If the model lacks a small helper needed for workspace index jumps, add it in `umux-core` or implement the lookup in `umux-ui` without changing persisted data shape.

Deferred actions should be explicit in tests so they are not mistaken for accidental omissions:

- `open_browser_split`
- `focus_address_bar`
- `show_notifications`
- `clear_scrollback`
- `toggle_sidebar`
- `settings`

## Data Flow

Startup flow becomes:

```text
SessionStore::load_model
  -> startup classification
  -> AppController::from_restored_model or AppController::new
  -> optional StartupWarning
  -> shell_view
  -> visible non-blocking warning banner
```

Shortcut flow becomes:

```text
Floem key event
  -> chord_from_key_event
  -> shell shortcut mapper
  -> AppAction or intentional defer
  -> AppController::apply
  -> session save and shared model sync
```

Diagnostics flow becomes:

```text
umux-ui::run
  -> init diagnostics
  -> floem::launch(app_view)
  -> traced startup, action, shortcut, and persistence events
```

## Reliability Requirements

- Do not change session file schema.
- Do not add blocking startup dialogs.
- Do not remove the existing corrupt-session rename-aside behavior.
- Do not implement IPC or WebView in this slice.
- Keep terminal copy/paste behavior routed through existing terminal input code.
- Keep the app usable even when logging initialization fails or is already initialized.
- Preserve current `cargo fmt`, `cargo test --all`, and clippy status.

## Testing Strategy

Automated tests should cover:

- session startup classification for missing, corrupt, unsupported schema, I/O failure, and controller restore failure where practical
- visible warning state is passed into the shell view
- runtime shortcut mapping for newly wired shell shortcuts
- explicit deferred shortcut actions so default declarations do not silently imply implemented behavior
- existing terminal input tests for copy/paste continue to pass
- existing session store, controller, terminal, and UI tests continue to pass

Required checks:

- `cargo fmt --check`
- `cargo test --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- GUI launch smoke

## Acceptance Criteria

This slice is complete when:

- GUI startup initializes diagnostics without panicking
- session load and restore failures are logged with useful context
- users see a compact warning when previous session state could not be restored
- save failures are logged instead of silently swallowed
- safe declared shell shortcuts are implemented or intentionally deferred in tests
- default shortcut declarations and runtime handling have test coverage against drift
- terminal copy/paste behavior remains unchanged
- automated checks and a GUI launch smoke pass

## Out Of Scope

- named-pipe IPC server and real `umux-cli` transport
- real WebView2 browser rendering
- browser automation command execution
- settings UI
- notification center UI
- address bar focus UI
- clear-scrollback shortcut plumbing if it requires broader terminal control ownership changes
- session schema migration
- redesigning the shell layout

## Implementation Notes

Suggested implementation slices:

1. Add diagnostics initialization and logging dependencies needed by `umux-ui`.
2. Introduce a small startup state type that carries `AppController` plus optional warning text.
3. Replace silent session load/restore fallbacks with explicit startup classification and tests.
4. Render the optional startup warning in the shell.
5. Expand shortcut handling for safe shell-level actions and add tests for handled versus deferred defaults.
6. Run format, tests, clippy, and GUI launch smoke.

## Spec Self-Review

- Placeholder scan: no placeholder markers remain.
- Internal consistency: the design consistently targets release hardening and keeps IPC/WebView out of scope.
- Scope check: the slice is focused enough for one implementation plan because it mostly touches diagnostics, startup classification, warning display, and shortcut mapping.
- Ambiguity check: startup failure behavior, deferred shortcut actions, testing expectations, and acceptance criteria are explicit enough to plan implementation without guessing.
