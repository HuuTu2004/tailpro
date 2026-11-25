# Contributing to TailPro

TailPro is an early desktop project for investigating remote logs over SSH. Contributions that make a real debugging workflow easier are welcome, including documentation and small, reproducible bug reports.

English and Vietnamese are both welcome. Most design and acceptance notes are currently in Vietnamese; ask for clarification when a requirement is unclear.

## Start with a concrete problem

- For a bug, describe what you did, what you expected, and what happened. Include your Linux distribution, relevant tool versions, source type, and a minimal synthetic or redacted example.
- For a feature, describe the debugging task and the limitation you encounter today. Discuss larger changes before implementing them.
- For documentation, a focused correction, setup clarification, or English translation is a useful contribution.

Do not include host credentials, private keys, enrollment tokens, real customer data, or unredacted production logs in reports. General workflow questions can go to [Tu Nguyen's discussions](https://github.com/HuuTu2004/HuuTu2004/discussions).

## Set up a development environment

Follow the [README prerequisites and setup](README.md#run-from-source). For a headless core check, a WebView is not required:

```bash
cargo check -p log-core --lib
cargo check -p app-core --lib
```

For the desktop app, install the Tauri Linux prerequisites, then run:

```bash
npm ci
npm run contracts
npm run tauri dev
```

## Keep the scope clear

Read the [current status](docs/process/current-status.md), [architecture](docs/system-design/architecture.md), and the relevant [design contracts](docs/system-design/contracts.md) before changing behavior. Pick a focused change; use the numbered roadmap item when applicable. Keep public claims consistent with the actual validation evidence.

Dependencies flow from desktop to `app-core` to `log-core`. Keep UI, SSH, and async runtime concerns outside the log engine. Preserve bounded resource use, host-key checks, source grants, redaction, and cancellation behavior.

Use rustfmt for Rust and the existing TypeScript/React style. Rust is the source of truth for generated frontend contracts; regenerate them with `npm run contracts` when their definitions change.

## Validate the affected area

The repository currently uses builds, lint, type checks, manual acceptance, and profiling. **Do not add automated tests or test infrastructure unless explicitly requested.** This is the current project policy, also recorded in [AGENTS.md](AGENTS.md).

Choose the checks relevant to your change:

```bash
cargo fmt --all --check
cargo check -p log-core --lib
cargo check -p app-core --lib
cargo clippy -p log-core -p app-core --lib -- -D warnings
npm run typecheck
npm run build
```

Check affected reverse dependencies when changing a public interface. For desktop or MCP behavior, also build the relevant binary and perform the scoped manual checks described in the acceptance notes. Documentation changes need link and content checks rather than a full application build.

Report the checks you ran, what you observed, and any checks you could not run. Keep local fixture results separate from real-host or release compatibility claims.

## Submit a focused change

Use a branch and a descriptive Conventional Commit, for example `docs(readme): clarify Linux setup` or `fix(viewer): preserve the paused selection`.

Explain the problem, resulting behavior, validation, and remaining limitations. Include a screenshot for a visible UI change or measurements for a performance claim. Keep feedback respectful and specific; discuss the work and the evidence.
