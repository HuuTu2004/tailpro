<p align="center">
  <img src="docs/assets/tailpro-banner.svg" alt="TailPro — your servers, your logs, one workspace." width="100%" />
</p>

<p align="center">
  <strong>A local-first desktop log explorer for developers who debug over SSH.</strong>
</p>

<p align="center">
  <a href="#why-tailpro">Why TailPro</a> ·
  <a href="#what-you-can-explore">Features</a> ·
  <a href="#run-from-source">Run from source</a> ·
  <a href="CONTRIBUTING.md">Contribute</a> ·
  <a href="README.vi.md">Tiếng Việt</a>
</p>

> **Development preview.** TailPro is under active development, with Linux as the initial target. Runtime validation is ongoing. Packaged AppImage/deb releases and release certification are still ahead. See the [current status](docs/process/current-status.md) for verified behavior and open validation work.

## Why TailPro

Debugging a service often starts with several SSH sessions, a few `tail -f` commands, and logs scattered across files and containers. TailPro brings those sources into one desktop workspace so you can search, inspect, and share the context you need.

It connects through **OpenSSH** and does not install a TailPro agent on your servers. Reading remote files requires Python 3 on the host; Docker sources require the Docker CLI and permission to read container logs.

## What you can explore

| Workflow | What TailPro provides |
| --- | --- |
| Connect to a host | SSH config aliases, existing SSH identities/agents, trusted host-key checks, and connection reuse. |
| Find log sources | Scoped discovery of Docker containers and log files, explicit source selection, and pinned paths. |
| Follow several sources | A combined log viewer, source health, pause/resume, and visible continuity or retention gaps. |
| Investigate an event | Filters, Saved Views, and a JSON/raw Inspector with surrounding context. |
| Share an incident | Preview redacted NDJSON or spreadsheet-safe CSV before exporting. |
| Give a coding client context | A scoped MCP bridge with explicit source grants and eight scoped evidence tools (schema 2). |
| Explore an explanation | Optional error explanations through local Ollama or a configured OpenAI-compatible BYOK endpoint. |

These workflows have implementation and local validation evidence, with remaining integration work documented in the [status report](docs/process/current-status.md). Model inference and provider compatibility still need runtime validation. Microservice debugging now includes Journal, error clusters, service/code hints, trace events/durations and opt-in static topology; its runtime gates and Linux release hardening are tracked in the [roadmap](docs/plan/roadmap.md).

## A typical session

1. **Connect** to a host using your existing OpenSSH setup.
2. **Discover** sources within a chosen scope, or enter an absolute file path.
3. **Start** the sources you want to follow.
4. **Filter and inspect** events; pause the viewport when you need to read.
5. **Export** a previewed, redacted incident or grant a compatible MCP client access to selected sources.

Discovery and streaming are separate actions. Pinning a path does not automatically start reading it.

| Shortcut | Action |
| --- | --- |
| `Ctrl+K` | Open the command palette |
| `Ctrl+F` | Focus the query field |
| `Space` in the log table | Pause or resume |
| `Enter` on a row | Open the Inspector |
| `Escape` | Close the active panel |

## Run from source

### Prerequisites

- A Linux desktop with the [Tauri system dependencies](https://v2.tauri.app/start/prerequisites/), including GTK 3 and WebKitGTK 4.1.
- Rust stable with Rust 2024 edition support.
- Node.js 20.19+ or 22.12+ and npm, matching the Vite 7 toolchain.
- OpenSSH on your machine and a reachable SSH host with an already trusted host key.
- Python 3 on hosts used for file reading/discovery; Docker CLI access for Docker sources.

From the project directory:

```bash
npm ci
npm run contracts
npm run tauri dev
```

To build a development binary with bundled frontend assets:

```bash
npm run build
cargo build -p tailpro-desktop --features custom-protocol --locked
```

This produces a development binary, not a packaged release. The Rust core libraries can be built without a WebView; see [CONTRIBUTING.md](CONTRIBUTING.md) for scoped checks.

## Your data and your tools

- **Local-first storage.** Settings are stored locally; retained logs and checkpoints are currently held in RAM.
- **Existing SSH trust.** TailPro uses trusted host keys and existing SSH identities. Dynamic SSH configuration requires session consent.
- **Explicit sharing.** Exports and external tool/model access use redaction, previews or grants as applicable. Review the material you share, especially real incident logs.
- **Optional model access.** Explain Error is disabled by default. You choose a local Ollama endpoint or an HTTPS BYOK endpoint and approve the prepared request. There is no TailPro model proxy or automatic cloud fallback.

See the [design contracts](docs/system-design/contracts.md) and [validation evidence](docs/process/current-status.md) for the intended boundaries and their current validation status.

## MCP: log evidence for coding tools

Build the adapter:

```bash
cargo build -p tailpro-mcp
```

Start a source in the desktop app, open **AI & MCP settings**, select the sources and client, and confirm the grant. Use the enrollment configuration shown by the app, replacing its command with the absolute path to your `tailpro-mcp` binary. Keep enrollment tokens private.

The current tools are `list_sources`, `get_recent_logs`, `search_logs`, `get_log_context`, `locate_service`, `get_error_clusters`, `trace_id`, and `inspect_context`. They expose scoped, redacted log evidence; the coding client owns model access, edits, builds, and commands. Session grants expire and must be renewed after a restart or source configuration change.

The [MCP and AI acceptance notes](docs/process/phase-03-acceptance.md) cover onboarding, observation cursors, limits, and remaining client/provider checks. Detailed operational notes are also available in the [Vietnamese README](README.vi.md#ai--mcp-và-incident-output-phase-3).

Journal requires Python 3, journalctl and permission to read the selected unit. Service mappings declare checkout hints without opening repositories. Trace bars use explicit durations; event-only logs remain markers. Topology requires separate exact-file/root consent and supports a static Compose subset; unresolved imports/variables and missing log coverage remain visible. See [Phase 4 acceptance](docs/process/phase-04-acceptance.md) for limits and remaining runtime checks.

## Help shape TailPro

Useful contributions include clearer setup instructions, reproducible source/SSH issues, Linux compatibility reports, and concrete debugging workflows. Please read [CONTRIBUTING.md](CONTRIBUTING.md) before starting a code change.

For early feedback and questions, [join the conversation with Tu Nguyen](https://github.com/HuuTu2004/HuuTu2004/discussions). English and Vietnamese are welcome. Share synthetic or redacted logs.

## Project map

| Read this | To understand |
| --- | --- |
| [Current status](docs/process/current-status.md) | What has been checked and what remains open |
| [Architecture](docs/system-design/architecture.md) | Code structure, ownership, and resource budgets |
| [Design contracts](docs/system-design/contracts.md) | Expected behavior and security boundaries |
| [Roadmap](docs/plan/roadmap.md) | The five development phases and their acceptance gates |
| [Build guide](docs/plan/build-performance.md) | Scoped compilation and build measurements |
| [Contributor guide](CONTRIBUTING.md) | Setup, change scope, and validation |
| [Agent instructions](AGENTS.md) | Repository conventions for coding agents |

Most engineering notes are currently in Vietnamese. English documentation improvements are welcome.

Built with **Rust · Tauri · React · TypeScript · OpenSSH** by [Tu Nguyen](https://github.com/HuuTu2004).
