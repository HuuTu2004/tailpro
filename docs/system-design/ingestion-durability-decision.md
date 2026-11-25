# Ingestion durability — decision 6.11

25/11/2026, Asia/Bangkok. Status: **Deferred**. Phase 6 is implemented in RAM mode; this work does not introduce a storage requirement.

Admission ACK means a bounded unit reached retained RAM or an explicit gap disposition. File checkpoints can describe a verified byte offset and partial-record range. Docker and Kubernetes resume from a complete transport timestamp with overlap; Journal resumes from an opaque completed entry cursor. None survives a desktop process crash as a durable guarantee. A reconnect does not prove complete upstream history, and replay is retained.

A disk spool would need a separate opt-in design and evidence for encryption/privacy and directory ownership, quotas and retention, versioned record/checkpoint recovery, atomic commit, fsync frequency, shutdown/cancellation, ENOSPC, corruption and torn writes. Its durable ACK must be after the selected write/fsync boundary, independently of UI consumption. A cursor without durable payload is insufficient. Recovery must expose uncertain ranges and honor source identity, grants, policy revisions and secret deletion.

No database, index, WAL, remote daemon or automatic disk log storage is enabled. This decision leaves Gate C unclaimed and does not block RAM ingestion. Revisit when a concrete required retention period, crash-recovery objective and disk budget are supplied. Gate A/B still require their own runtime evidence.
