# Tiến độ hiện tại

Cập nhật: 20/11/2026, Asia/Bangkok. Snapshot implementation tại workspace `tailpro`; docs ở `docs/`.

| Mục | Trạng thái thực tế |
| --- | --- |
| Scope vừa triển khai | Phase 4, code 4.1–4.7 theo yêu cầu toàn file; giữ các prerequisite gates Phase 1–3 |
| Code | Rust/Tauri/React viewer + Journal, tám MCP tools/schema 2, service mapping/code hints, trace duration/event UI và opt-in static topology |
| Validation | Core/adapter/native clippy/build, frontend build; synthetic grouping/correlation/caps/ACK/Compose probes, raw MCP + Codex tools, Claude handshake; xem acceptance từng phase |
| Phase gate | **PARTIAL — chưa fully accepted**; không coi feature code là runtime/release certification |
| Quy tắc | Không tạo automated tests; dùng build/lint/typecheck/manual smoke/profiling |

## Phase 1 — Prerequisite gates

| Mục | Code / local evidence | Gate còn thiếu |
| --- | --- | --- |
| 1.1 | Workspace/core headless/typed health/React/Tauri, native window exit 0 | Ubuntu 24.04 base/runtime matrix và cold desktop timings |
| 1.2 | Static Include/alias import, scoped dynamic consent, strict host keys, owned mux, cancel/reap/backoff; unknown/changed keys và config invalidation local | ProxyJump/IdentityAgent/MaxSessions và mất mạng trên host thực |
| 1.3 | Bounded frame/JSON selective parser/ANSI/multiline/quality/opaque string IDs, manual large/deep/malformed/Python stack | Matrix Go/Node/late/stop edge cases đầy đủ |
| 1.4 | Owner worker/DRR/byte+row reservations/pinned accounting, atomic capture/coalesced subscription/incomplete journal/CPU permits; 100k/500k và pinned pressure có số đo | Sustained 15k/s+3 quiet sources, UI freeze/RSS thật; zero-drop rerun sau preview fix |
| 1.5 | Typed Docker logs, separate streams, contiguous checkpoint, 1 ns replay overlap, replay classification, bounded retry | Mới chạy CLI stub; chưa Docker daemon thật/restart/recreate/downtime/permissions |
| 1.6 | Descriptor-first helper/byte offset+guard/generation, quote+space path, rotation/truncate và 5 GiB sparse seek đã chạy | FIFO race/revoke/copytruncate ambiguities và remote CPU matrix đầy đủ |
| 1.7 | Docker/units/PID FD/config opt-in, shallow scan, literal completion/latest-wins, growth evidence, pins/tags/provenance | Large directory/denied/cancel/providers và includes/symlink matrix trên target thực |
| 1.8 | Allowlisted templates, no arbitrary exec API, Production labels qua connection/source/preview; separate log/config consent | Tiếp tục reuse policy/labels ở UI/MCP phase sau |

## Phase 2 — Theo mục

| Mục | Code / local evidence | Gate còn thiếu |
| --- | --- | --- |
| 2.1 | Typed paged snapshots, virtual rows, 2000-row cache, metadata polling/ACK, health/gaps/Production; 150.500 records khi consumer freeze 30 s | Native loaded UI/RSS/scroll và incident qua 2 host/4 sources thật |
| 2.2 | Literal/AND/fields/duration compiler, deadline/cancel/latest generation, Saved Views bounded; core p95 7,87–70,82 ms trên 100k rows | Native E2E p95, sustained interactive matrix trên nguồn thật |
| 2.3 | Stateless lazy JSON/raw/group parts, explicit copy, bounded detail cancellation; 5000-key paging và escaped UI fixture | Deep/giant raw, clipboard và stack-part eviction matrix đầy đủ |
| 2.4 | Paused cached viewport, opaque reading anchor, count từ notices, older pages/resume/fallback; browser pause/expired anchor smoke | Pause + ingest/eviction/Inspector/filter trên native nguồn thật |
| 2.5 | Palette reuse completion/explicit Start, Ctrl+K/F, viewport Space/arrows/Enter, Escape/focus restore | Linux accessibility/keyboard/provider-cap matrix đầy đủ trên nguồn thật |

Evidence commands, fixture và giới hạn ở [acceptance phase 2](phase-02-acceptance.md). Code đã có cho 2.1–2.5; gate tích hợp **PARTIAL**, không tự coi target performance/agent-ready đạt.

## Phase 3 — Theo mục

| Mục | Code / local evidence | Gate còn thiếu |
| --- | --- | --- |
| 3.1 | Shared bounded fail-closed projection; nested/URL/userinfo/Bearer/JWT/JWE/huge canaries và secret-only equivalence | Adversarial/resource matrix rộng hơn; conservative masking có thể giảm nội dung |
| 3.2 | Exact prepared NDJSON/CSV, metadata, TTL/digest/epoch, private atomic native destination; eviction/61 s TTL/revoke/cancel/disk failure/CSV đã quan sát | Native destination/cancel/fs failure matrix đầy đủ |
| 3.3 | app-protocol, rmcp 0.8.5 / MCP 2025-06-18, private socket/lock/registry, onboarding/grants; Codex 0.170.0 và Claude 2.1.304 handshake, SIGKILL/restart/CLOEXEC/preauth/frame caps | Full socket/owner/backlog/race và shared 8 MiB/RSS measurement |
| 3.4 | Bốn scoped redacted tools, opaque refs, caps/rate; Codex search/context thật, raw MCP smoke, unauthorized/revoke | Claude tool calls cùng incident; concurrency/revoke-race/stress |
| 3.5 | Metadata baseline, forward fence/pages 3/3/1 không skip, backward recent, scoped generation-bound TTL handles, gaps | Cancel/single-flight/query/generation/gap race matrix đầy đủ trên clients thật |
| 3.6 | Disabled-by-default Ollama/BYOK, exact preview/consent, no proxy/redirect/cloud fallback, OS store fail-closed, untrusted hypothesis UI; HTTP policy fixture | Không có Ollama/model hoặc secret-tool trên máy; local inference và BYOK provider/store thật chưa accepted |

Evidence/commands/caps alpha cụ thể ở [acceptance phase 3](phase-03-acceptance.md). **PARTIAL**; không dùng feature code để tuyên bố hoàn tất runtime/release gates.

## Phase 4 — Theo mục

| Mục | Code / local evidence | Gate còn thiếu |
| --- | --- | --- |
| 4.1 | Journal reader, opaque contiguous ACK cursor, normalized journal/priority/time, bounded recent fallback/gaps; helper/ACK probes | Journal daemon/SSH/quyền/vacuum/disconnect thật |
| 4.2 | Deterministic sanitized clusters, full representation collision comparison, 18/6 counts; 500-sample/128-group caps, payload 3.078 B vs raw 14.769 B | Broad collision/resource/retention matrix, repeated incident p95 |
| 4.3 | Persisted developer mappings, no repo I/O; relative stack hints, hostile paths rejected, revision unverified | Stack format/adversarial/mapping race matrix đầy đủ |
| 4.4 | Scoped literal source/service/project/host aliases; two candidates ambiguity, out-of-grant no leak, no Start | Two-host real metadata/availability matrix |
| 4.5 | Typed trace/order exact match, scoped trace refs, inspect evidence/group/trace; schema 2/eight tools, raw MCP + Codex calls | Claude domain calls, full expiry/cancel/revoke/generation races |
| 4.6 | Source-grouped explicit duration bars/event markers, click evidence, cap/cancel/view reuse; browser fixture | Loaded native clock-skew/missing-parent/partial-retention matrix |
| 4.7 | Separate exact-file/root consent, static Compose JSON/block-YAML subset, hostname-only hints, bounded graph/counts/provenance | Remote/native resource matrix; runtime env unsupported, metadata revision polling 15s invalidates changed config |

Evidence/commands và giới hạn ở [acceptance phase 4](phase-04-acceptance.md). **PARTIAL**; topology metadata freshness đã có bounded polling 15s; không dùng code/CLI handshake để thay runtime gates.

## Evidence chính

Xem [acceptance phase 1](phase-01-acceptance.md) để biết commands, version, fixture và giới hạn.

- Hai sshd loopback endpoints/bốn file sources, consumer dừng 30 s; warm open 33–43 ms (4 samples), shutdown 4 ms.
- Headless file soak: 600k synthetic records/600 s, có config-generation reconnect; peak local process-tree RSS 234,74 MiB. 704 explicit pressure dispositions với preview cũ, chưa zero-drop pass sau bounded-suffix fix.
- Core 500k input: accepted 500.201 gồm quiet records; byte cap giữ 170.634 rows, quiet sources vẫn retained; latest-50 single sample 444 µs. Đây là dev/allocator-instrumented profile, không phải release target.
- Desktop build dùng dev packages extract vào `/tmp` vì sudo yêu cầu interactive auth. Không cài package/daemon vào VPS hoặc sửa SSH/known_hosts thật.

## Next action

### UI và connection manager ngày 20/11/2026

Màn hình chính tập trung vào log, chỉ giữ mục Connections và bỏ chữ `/ local`. Connections và Settings là hai cửa sổ native riêng, mỗi cửa sổ mặc định 760×600; New connection là cửa sổ hệ điều hành riêng 600×560, có Test connection, Save và Cancel. Các cửa sổ có thanh tiêu đề và nút đóng hệ thống; Connections/Settings không còn nút Close trùng trong nội dung. Save chỉ lưu profile; Connect từ danh sách saved vẫn tường minh. Bấm Connections chỉ mở Connections; bấm bánh răng Settings chỉ mở Settings. Sources/Trace/Topology mở theo ngữ cảnh từ màn hình log. Không còn trang cấu hình workspace/Log explorer trên sidebar.

Đã chạy frontend typecheck/build, app-core và desktop clippy/check/build; browser smoke kiểm tra query/pause, dialog Escape/focus, raw escaping, export cancel và layout 900×600; native đã mở được cửa sổ quản lý riêng. Probe thủ công xác nhận refused/cancel không lưu profile, không thêm host/source hay đổi policy; Save lưu một profile nhưng active hosts vẫn bằng 0; không còn SSH child sau cleanup. Preview hiện chạy bằng Vite development với `?preview=1`: dữ liệu mẫu có nhãn Demo, data IPC không đi tới runtime thật; chỉ thao tác mở/đóng cửa sổ vẫn native. Nhánh preview bị loại khỏi production build. Table và Event details đã chung một khung, mở bằng Details/Enter/double-click, divider kéo/keyboard được và có scrollbar ngang cố định. Browser smoke xác nhận box 1014px: mặc định table 1014px, mở details 390px/table 616px, kéo thêm 100px cho details thì table còn 516px, đóng trả lại 1014px; scrollbar và viewport đồng bộ scrollLeft. Saved views đã bỏ khỏi toolbar. Không viết automated tests; SSH authentication thành công trên server thật và các runtime gates trước đó vẫn chưa được chứng nhận bởi đợt UI này.

Đóng các runtime gates phase 1–2 trên target/native thật. Với phase 3: chạy Claude domain tools/observation, Ollama local model và BYOK/OS Secret Service thật; hoàn thiện race/resource/socket matrix và đo shared bridge/query pool + process-tree RSS. SDK/protocol đã pin theo handshake local, chưa tuyên bố compatibility/performance rộng hơn. Với phase 4: Journal/Compose target thực, loaded native graph/trace, Claude specialized calls và full cancel/retention/resource matrix; topology freshness/race matrix cần kiểm chứng trên target. Phase 5 chưa triển khai; chưa phát hành AppImage/deb.
