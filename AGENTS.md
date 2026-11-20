# Repository Guidelines

## Project Structure & Module Organization

Phase 1 has a Rust workspace and Tauri/React desktop shell. Consult `docs/process/current-status.md` for validated behavior and remaining gates.

- `crates/log-core`: pure parsing, immutable segments, reservations, snapshots and journal.
- `crates/app-core`: application API, OpenSSH, source readers, discovery and settings.
- `src-tauri`: composition root and native commands; `src`: React connections/sources UI.
- `crates/*/examples`: manually invoked profiling/acceptance drivers; synthetic inputs in `fixtures/`.

- `docs/process/current-status.md`: current progress, blockers, and next action.
- `docs/system-design/architecture.md`: code map, ownership, dependencies, and budgets.
- `docs/system-design/contracts.md`: required discovery, ingestion, recovery, MCP, and security behavior.
- `docs/plan/roadmap.md`: five phases, integration gates, and performance targets.
- `docs/plan/phase-0N-*.md`: numbered implementation items with scope and completion criteria.
- `docs/plan/build-performance.md`: compilation profiles, caching, and timings.

Before coding, select the requested numbered item, such as 1.7, and read its design contracts. Record missing prerequisites; implement the selected scope, batching items only when requested. Dependencies follow `desktop → app-core → log-core`; keep Tauri, Tokio, and SSH outside the engine. MCP reuses the application. External coding clients own model access, code edits, and commands.

## Build and Development Commands

Choose the affected package:

```bash
cargo fmt --check
cargo check -p log-core --lib
cargo clippy -p log-core --lib -- -D warnings
cargo build -p log-core --lib
```

Check reverse dependents after public interface changes. Build binaries for runtime/linking validation; use actual frontend scripts. Preserve incremental caches; reserve workspace/release builds for integration/distribution. Follow the compile guide; documentation edits require link/content checks.

## Coding Style & Simplicity

Use kebab-case Markdown filenames and stable numbered item IDs. Follow rustfmt's four-space Rust style and two-space TS/TSX indentation; snake_case Rust functions, PascalCase React components, camelCase TypeScript functions. Generate wire DTOs from Rust.

Implement the smallest readable solution for current requirements. Reuse modules; add abstractions, crates, or dependencies for demonstrated needs. Minimize hot-path allocations, copies, repeated parsing, and serialization. Bound buffers, queues, caches, and concurrency. Keep CPU-heavy work off async executors; profile before indexes, SIMD, parallelism, or caching. Preserve correctness and resource accounting.

## Validation Policy

Write no automated tests or test infrastructure unless explicitly requested. Use scoped build, lint, typecheck, manual acceptance, existing relevant checks, and profiling. Record unrun checks and limitations honestly.

## Commit & Pull Request Guidelines

Git history is absent. Use Conventional Commits, e.g. `docs(plan): group work into five phases`. PRs explain the problem, behavior, numbered item, validation, and limitations; include screenshots or measurements when relevant.

## Security & Configuration

Preserve host-key checks, consent, budgets, redaction, and grants. Keep real logs, secrets, and private keys out of commits/diagnostics. Update authoritative design and the affected plan when behavior changes; process records actual progress.
