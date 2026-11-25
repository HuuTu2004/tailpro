# Parser configuration v1

The Sources panel imports a local `.tailpro.toml` selected by the user. Preview shows normalized fields, conflicts and capture spans for a supplied sample; Apply changes only the selected active source session. No remote config scanning, interpolation, scripts, plugins, repository opening or sample persistence occurs. Stopping the source ends its parser configuration; import again for a new session.

```toml
version = 1

[parse.profile]
time_unit = "milliseconds" # seconds | milliseconds | microseconds | nanoseconds
level_namespace = "pino"   # text | pino | syslog | open_telemetry | unknown
# timezone_offset_minutes = 420 # source timezone, never inferred from client
# access_log = true              # common/combined access status after quoted request
# raw_only = true                # prepare and preview rollback before Apply

[[parse.rules]]
id = "team-text"
source = "source-SELECTED-ID"
pattern = '^(?P<time>\S+)\s+(?P<level>INFO|WARN|ERROR)\s+(?P<msg>.*)$'
time_format = "rfc3339"
```

Copy the selected Source ID displayed in the panel into each rule's `source`. Maximum config size is 64 KiB, 4 candidate rules per source, 16 across the active workspace; rule IDs are unique and at most 64 bytes, patterns at most 2 KiB, at most 8 capture groups. Regex program/DFA limits are 96/32 KiB per rule, with room within the 4 MiB control ceiling for old/new configuration overlap and source text. Compile occurs in the existing bounded CPU worker pool, outside the live ingestion executor. Invalid imports leave the last valid configuration active.

Capture names: `time`, `level`, `msg`, `trace_id`, `order_id`, `user_id`, `duration_ms`. Rust regex does not support lookaround or backreferences. There is one capture search per candidate over at most 64 KiB; unmatched records fall through to the shared decoder. Fragments stay raw. A matched rule replaces default extraction and keeps its source parser revision. Revision/generation boundaries fence multiline state; previously published events remain immutable. Advanced custom numeric namespaces, configurable nested mapping paths and user-defined multiline state machines are not accepted by v1's schema.

Default structured paths and priority are in [ingestion schema](ingestion-schema.md). JSON validates the complete bounded record and only retains selected fields. Logfmt uses a quote/escape state machine; malformed suffixes invalidate extraction. `attr.http.response.status_code:502` queries exact scalar values at a JSON dot path; logfmt uses the exact key following `attr.`. Paths are bounded to 128 bytes/16 components. Array indexing, JSONPath expressions, regex queries and recursive leaf matching are unsupported. Inspector reads other JSON/logfmt attributes lazily.

Numeric epoch timestamps require an explicit unit; numeric severity requires a namespace. Nanoseconds are checked integers and remain strings in evidence. Missing year/timezone remains unresolved; timezone offsets here are fixed offsets, not an IANA/DST resolver. Header interpretation is conservative; unexpected formats retain raw bytes and Unknown fields. Access-derived HTTP error is separate from logger severity. Raw-only fallback still obeys retention, gaps and external output policy.
