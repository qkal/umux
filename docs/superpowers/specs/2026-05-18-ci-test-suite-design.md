# CI Test Suite Design

Date: 2026-05-18

## Goal

Improve the project's GitHub Actions coverage with a Windows-first CI suite and a targeted Linux lane for platform-neutral crates. The suite should catch regressions in formatting, linting, core model behavior, IPC contracts, session restore, browser automation state, CLI request generation, and terminal component behavior without making every pull request noisy or unnecessarily slow.

## Current Context

`umux` is a Rust workspace and is explicitly Windows-first. The repository already has a `.github/workflows/ci.yml` workflow that runs on `windows-latest` for pushes and pull requests to `main`. It installs stable Rust with `rustfmt` and `clippy`, then runs:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`

The workspace includes Windows-specific crates and GPUI/UI crates, plus portable logic crates such as `umux-core`, `umux-ipc`, `umux-session`, `umux-notify`, `umux-browser`, `umux-cli`, and `umux-config`. Existing tests already cover many unit-level paths, plus integration tests for core foundation flow, IPC request flow, and Windows ConPTY behavior.

## Recommended Approach

Keep Windows as the authoritative CI target and add a smaller Linux lane for crates that should remain portable.

The workflow should be split into clear jobs:

- `quality`: Windows. Run formatting and clippy for the workspace.
- `windows-tests`: Windows. Run the full workspace test suite with the checked-in lockfile.
- `linux-portable-tests`: Ubuntu. Run tests only for portable crates: `umux-core`, `umux-ipc`, `umux-session`, `umux-notify`, `umux-browser`, `umux-cli`, and `umux-config`.

This keeps the primary signal aligned with the app's Windows-first target while still catching accidental portability regressions in logic crates.

## New Test Coverage

Add a small set of useful tests instead of duplicating existing assertions:

- Browser state: verify back/forward history truncation after opening a new URL from the middle of history.
- CLI contract: verify commands such as `split`, `ping`, and browser open map to stable IPC methods and params.
- IPC contract: verify error responses round-trip with structured error shape and that malformed response bodies are rejected.
- Session/model behavior: verify restored unread metadata continues with the correct next unread sequence and that stale workspace unread state is recomputed from surfaces.

Prefer colocated unit tests when the behavior belongs to one crate. Prefer integration tests only when the behavior crosses crate boundaries.

## Data Flow

The CI flow is:

1. Checkout repository.
2. Install stable Rust and required components from the existing toolchain configuration.
3. Use Cargo's lockfile with `--locked` for deterministic dependency resolution.
4. Run Windows quality and full tests.
5. Run Linux tests only for explicit portable packages.

The test data flow stays local to each crate. Tests should create temporary data under OS temp directories when filesystem state is needed and should avoid depending on user-specific paths except where existing Windows-focused tests already do.

## Error Handling

CI should fail fast within each job but keep jobs independent so Linux portable failures do not hide Windows full-suite failures. Cargo failures should be allowed to surface directly because they already provide actionable compiler, lint, or test output.

Filesystem tests should clean up or isolate temporary paths. Existing user changes and untracked local files are out of scope and should not be touched.

## Alternatives Considered

One alternative is a maximal Windows/Linux matrix for most workspace crates. It gives broader coverage but is likely slower and noisier because UI, GPUI, and Win32 crates have platform-specific dependencies.

Another alternative is Windows-only hardening. It is simpler and fastest for a Windows-first app, but it misses regressions in portable logic crates that can compile and test cleanly on Linux.

## Acceptance Criteria

- GitHub Actions keeps Windows as the primary full-suite target.
- A Linux job tests only portable crates.
- CI uses `--locked` for reproducible dependency resolution.
- New tests add meaningful behavioral coverage for browser navigation, CLI-to-IPC request contracts, IPC response contracts, and session/model restore behavior.
- Existing user changes outside the CI/test work remain untouched.

