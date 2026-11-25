# Tiến độ hiện tại

Cập nhật: 25/11/2026, Asia/Bangkok. Snapshot implementation tại workspace `tailpro`; docs ở `docs/`.

| Mục | Trạng thái thực tế |
| --- | --- |
| Source cleanup | Runtime dùng native IPC và adapters thật; đã gỡ demo backend, fixtures/examples, synthetic source seeding và endpoint preview cũ. `npm run contracts` dùng generator trong app-protocol; số đo lịch sử không thay runtime gates |
| Scope vừa triển khai | UI interface-refactors B1–B5 đã có code, browser review + native bundle smoke; [nghiệm thu UI](interface-refactors-acceptance.md) PARTIAL. Phase6: original bytes/fragments, typed JSON/logfmt/header mapping, six-family multiline, manual parser rules, provenance/metrics/query coverage và scoped Kubernetes; simulation được user chấp thuận; các prerequisite/runtime gates trước đó giữ riêng |
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

## Phase 6 — Schema và universal ingestion

| Mục | Trạng thái / evidence | Gate còn thiếu |
| --- | --- | --- |
| 6.1 | Schema v1 chốt; checked borrowed payload ranges, canonical nanosecond strings, three-clock display selection, severity namespaces/origin và shared disposition; Clippy/fmt/reverse-dependents check và manual walkthrough PASS | Foundations không tự chứng nhận runtime. Migration retained Event/readers/DTO và sửa nested-key/numeric-time ghi ở6.2–6.8 bên dưới; runtime gates vẫn riêng |
| 6.2 | Original byte ownership + bounded fragments, CRLF/EOF/interrupted, mid-record file checkpoint, entry-level Journal ACK; exact reconstruction và boundary literal observed | Large/eviction/replay/adversarial matrix rộng hơn |
| 6.3–6.5 | Deterministic aliases/numeric namespaces/checked ns; JSON/logfmt/header/access mapping, three clocks; 22 synthetic cases observed | Fixed timezone offset, chưa IANA/DST resolver; profile/custom format scope ghi rõ |
| 6.6 | Go/Python/Node/Java/PHP/Rust shared assembler; revision/generation fence, cap/late lineage | Interleaving không ID vẫn không thể tái tạo chắc chắn; native sustained chưa chạy |
| 6.7 | Explicit local TOML import/preview/apply, bounded regex, source-session revisions và raw-only rollback | Không persist source-session config; advanced custom mapping/multiline schema Unsupported |
| 6.8 | Inspector original hex/provenance/fragment coverage, local source counters, scoped MCP sanitized counters/provenance và query gaps | Telemetry proposal chưa triển khai toàn bộ histograms; UI đo sustained thật còn thiếu |
| 6.9 | Docker capability preflight, binary/repeated Journal, file resume/rotate/truncate helper simulation | Docker/Journal daemon/quyền/driver/vacuum/reconnect thật Unrun |
| 6.10 | 100k/500k before/after, level predicate không per-event formatting; fallback fair excess eviction sửa gap tại floor; 500k nhận đủ không admission gap | Metadata tăng charge, giảm retention rows; chưa chứng nhận performance E2E/native/remote |
| 6.11 | Deferred; [RAM/durability decision](../system-design/ingestion-durability-decision.md) | Không disk spool/WAL, không durable ACK |
| 6.12 | Selected Kubernetes context/namespace/Pod UID/container instance, sealed memfd config, no credential plugins, restart/config/error simulation | Gate B trên cluster thật Unrun; API name-based identity race vẫn uncertain |

Xem [schema contract](../system-design/ingestion-schema.md) và [acceptance6.1](phase-06-acceptance.md). Code đã triển khai trong phạm vi capability v1; tiếp tục đóng runtime/performance gates theo [evidence phase6](phase-06-acceptance.md). Không thay thứ tự đóng runtime/release gates phase1–5 đã ghi.

## Evidence phase 1–4

Xem [acceptance phase 1](phase-01-acceptance.md) để biết commands, version, fixture và giới hạn.

- Hai sshd loopback endpoints/bốn file sources, consumer dừng 30 s; warm open 33–43 ms (4 samples), shutdown 4 ms.
- Headless file soak: 600k synthetic records/600 s, có config-generation reconnect; peak local process-tree RSS 234,74 MiB. 704 explicit pressure dispositions với preview cũ, chưa zero-drop pass sau bounded-suffix fix.
- Core 500k input: accepted 500.201 gồm quiet records; byte cap giữ 170.634 rows, quiet sources vẫn retained; latest-50 single sample 444 µs. Đây là dev/allocator-instrumented profile, không phải release target.
- Desktop build dùng dev packages extract vào `/tmp` vì sudo yêu cầu interactive auth. Không cài package/daemon vào VPS hoặc sửa SSH/known_hosts thật.

## Next action

### Interface refactors B1–B5 — 25/11/2026

Đã triển khai font Plex WOFF2/OFL cục bộ, token/base và primitives chung; row32px/header28px/geometry thống nhất, gutter mức độ, timestamp phân cấp, highlight literal/message và footer24px. Sidebar240px, tabs36px gạch chân; Connections/Settings ở tiện ích cuối sidebar. Inspector400px có Fields/Raw/Stack + More, row copy actions, resize pointer/keyboard/reset và focus restore. Connections/Sources/parser/Settings/Trace/Topology/export/Explain theo cùng palette/control, bỏ CSS cũ. Minimum main window800×600; backend/DTO/security/budgets không đổi.

Frontend build/typecheck, desktop custom-protocol build và diff/docs checks PASS. Browser bốn kích thước với Inspector đóng/mở không page overflow; viewport679px ở1440×900 và547px ở1366×768, bỏ header vẫn đạt mục tiêu70%/65%. Test/Save không Connect, tab switch không Stop, × đúng reader; query/anchor expiry/reload, More/Evidence/Original bytes, clipboard arguments, pointer/keyboard resize, dialog/menu focus, reduced motion được quan sát qua adapter ngoài repo. Native bundle mở bằng config/runtime directory tạm, chưa đọc nguồn thật. Ảnh/specimen và chi tiết tại [interface acceptance](interface-refactors-acceptance.md).

**Nghiệm thu PARTIAL:** synthetic UI review không thay log thật đã ẩn danh và loaded native matrix; native resize capture có stale/black backing nên không dùng làm pass. Probe cuộn production browser200 rows,3 lượt trước/sau đều p95≈16,7–16,8ms; heap biến thiên, chưa native sustained/RSS certification. Tô cấu trúc message tùy chọn deferred; cần reviewer độc lập, native accessibility/clipboard/scaling/zoom và nguồn thật. Không thêm automated tests/dependencies/demo runtime. Các mục dưới đây là lịch sử các đợt UI trước, không phải mô tả vị trí Settings hiện tại.

### Cây HOSTS và tab reader — 25/11/2026

Shell đã thu gọn theo yêu cầu tiếp theo: bỏ hàng Logs/Sources/Investigate và mục Connections riêng, tab log một dòng nằm sát mép trên. Icon Settings ở góc phải thanh tab, + cạnh HOSTS vẫn mở Connections; menu ⋯ cuối navigator giữ công cụ phụ và palette. Review UI bằng adapter ngoài repo xác nhận tab strip top=0/height32px, tab31px, chỉ một Settings icon; tab trùng tên có host trên cùng dòng, tên khác nhau không thêm dòng phụ. Settings và + lần lượt gọi đúng section settings/connections; mở Trace qua menu phụ và Combine vẫn hoạt động. Layout900×600 khi paused không tràn ngang; frontend typecheck/build đạt.

Theo yêu cầu luồng giống DBeaver: đã thay hàng SourcePicker bằng cây host/log trong sidebar và các tab log đang đọc. Expand host hiện số log đã biết; Add log tạo path trong cây, chưa Start; double-click/Enter mới mở reader/tab. Nguồn đang mở được activate, có guard chống Start trùng. × gọi Stop thật qua native API; chuyển tab giữ reader. Combine chọn ít nhất hai nguồn đang mở, có thể trở lại nhóm sau khi xem một tab đơn. Đóng hết tab đóng view và dừng query, không vô tình query toàn bộ retained logs. File thêm tại cây chỉ pin sau streaming theo native policy; các descriptor khác được nhớ trong phiên (cap128), pinned paths đọc tối đa100/host. Không khôi phục backend demo/fixtures đã gỡ ở cleanup.

Frontend typecheck/build đạt. Review trình duyệt bằng adapter IPC tạm ở /tmp, chỉ inject trong DevTools, không nằm trong runtime/repo: thêm path và single-click có0 Start; double-click tạo1 reader/tab, double-click lại vẫn1; × có1 Stop và node giữ lại ở trạng thái closed; reopen tạo reader mới. Combine hai nguồn khác host gửi đúng hai source IDs; chuyển tab đơn rồi trở lại Combined giữ đúng nhóm. Đóng hết tab trả0 rows,1 close_view, không phát sinh view_page trong thời gian quan sát. Lỗi Start không tạo tab; lỗi Stop giữ reader/tab và báo lỗi, sau đó retry được. Layout1280×840 và900×600 không tràn ngang trang, cây còn hiển thị ở900px và scrollbar ngang nằm trong viewport. Đây là manual UI acceptance với synthetic IPC adapter ngoài repo, chưa xác nhận đọc/cleanup SSH trên server thật hoặc tương thích mọi source kind; backend guards/budgets không đổi trong phần UI này. Không viết automated tests.

### UI và connection manager ngày 20/11/2026

Màn hình chính tập trung vào log, chỉ giữ mục Connections và bỏ chữ `/ local`. Connections và Settings là hai cửa sổ native riêng, mỗi cửa sổ mặc định 760×600; New connection là cửa sổ hệ điều hành riêng 600×560, có Test connection, Save và Cancel. Các cửa sổ có thanh tiêu đề và nút đóng hệ thống; Connections/Settings không còn nút Close trùng trong nội dung. Save chỉ lưu profile; Connect từ danh sách saved vẫn tường minh. Bấm Connections chỉ mở Connections; bấm bánh răng Settings chỉ mở Settings. Sources/Trace/Topology mở theo ngữ cảnh từ màn hình log. Không còn trang cấu hình workspace/Log explorer trên sidebar.

Đã chạy frontend typecheck/build, app-core và desktop clippy/check/build; browser smoke kiểm tra query/pause, dialog Escape/focus, raw escaping, export cancel và layout 900×600; native đã mở được cửa sổ quản lý riêng. Probe thủ công xác nhận refused/cancel không lưu profile, không thêm host/source hay đổi policy; Save lưu một profile nhưng active hosts vẫn bằng 0; không còn SSH child sau cleanup. Trong đợt UI trước đã dùng Vite demo để quan sát layout; mock IPC/DEMO và `?preview=1` đã được gỡ ở đợt cleanup25/11/2026. Hiện mọi cửa sổ dùng native IPC thật. Table và Event details đã chung một khung, mở bằng Details/Enter/double-click, divider kéo/keyboard được và có scrollbar ngang cố định. Browser smoke xác nhận box 1014px: mặc định table 1014px, mở details 390px/table 616px, kéo thêm 100px cho details thì table còn 516px, đóng trả lại 1014px; scrollbar và viewport đồng bộ scrollLeft. Saved views đã bỏ khỏi toolbar. Không viết automated tests; SSH authentication thành công trên server thật và các runtime gates trước đó vẫn chưa được chứng nhận bởi đợt UI này.

Đóng các runtime gates phase 1–2 trên target/native thật. Với phase 3: chạy Claude domain tools/observation, Ollama local model và BYOK/OS Secret Service thật; hoàn thiện race/resource/socket matrix và đo shared bridge/query pool + process-tree RSS. SDK/protocol đã pin theo handshake local, chưa tuyên bố compatibility/performance rộng hơn. Với phase 4: Journal/Compose target thực, loaded native graph/trace, Claude specialized calls và full cancel/retention/resource matrix; topology freshness/race matrix cần kiểm chứng trên target. Phase 5 chưa triển khai; chưa phát hành AppImage/deb.

## Source cleanup — 25/11/2026

Đã loại bỏ Rust manual/profiling examples và synthetic fixtures, Python CLI simulation, raw observation dumps, `manual-fixtures`/`manual_source`, frontend demo backend và truyền cờ Demo giữa native windows. Bỏ endpoint preview cũ cùng DTO/worker/snapshot helper chỉ được driver cũ dùng; paged viewer/Inspector và preview parser/export/Explain thật vẫn giữ. TypeScript codegen chuyển vào `crates/app-protocol/src/bin/export-contracts.rs`, chạy bằng `npm run contracts`.

Các báo cáo acceptance lưu kết quả lịch sử, được đánh dấu rõ khi command/artifact đã bị loại bỏ. Các file nguồn UI đang phát triển (host log tree/source tabs), adapter Python chạy trên host thật và kiến trúc/bảo mật/grants không bị thay thế bằng stub. Cleanup không chứng nhận tương thích daemon/cluster hoặc readiness production; các runtime gates còn thiếu giữ nguyên.

Validation sau cleanup: `cargo fmt --all -- --check`, scoped Clippy cho log-core/app-core/app-protocol/MCP/desktop (`--lib --bins --locked -- -D warnings`), `npm run contracts`, `npm run build`, và build desktop bundled-assets (`--features tailpro-desktop/custom-protocol`) cùng MCP đều PASS. Tất cả30 module TS/TSX/CSS còn lại có đường import từ entry chính; không còn mock/demo/synthetic source hook trong runtime. Sáu Python helper thật compile syntax trong bộ nhớ; local Markdown links/fences và `git diff --check` PASS. Xóa thêm~543MiB executable examples cũ trong target/debug/examples; dependency/incremental caches giữ nguyên. Không kết nối hoặc thay đổi cấu hình host thật trong đợt cleanup.


## Tinh gọn menu và kết nối — 25/11/2026

Theo yêu cầu mới, desktop đã gỡ menu ⋯ và cả4 mục Find host or log, Sources, Trace, Topology. Bỏ Ctrl+K, Discover sources tại host và lối mở Sources từ empty state; bỏ tool panel và CSS shell liên quan. Cây HOSTS vẫn thêm absolute file path mà chưa Start, double-click/Enter mới mở log; Ctrl+F, tabs/Combine, Inspector, Connections và Settings vẫn hoạt động. Đây là thay đổi bề mặt desktop; không xóa backend ingestion/discovery/trace/topology hoặc thay quyền/grants.

Connections và + cạnh HOSTS mở thẳng native connection-editor với form SSH, không qua danh sách → Add. Hành động chính là Connect, không yêu cầu Save trước; API connect hiện có lưu profile và bắt đầu phiên, trạng thái/retry hiện trong cây HOSTS. Saved connection là dropdown điền form, chưa tự kết nối; Test connection vẫn riêng và không lưu profile/thay phiên đang chạy. Lỗi submit giữ bản nháp; Connect pending khóa input và chống submit trùng; Cancel không gọi Connect/Save. Settings vẫn mở cửa sổ riêng.

Validation: npm run build (gồm typecheck) và git diff --check PASS. Manual browser smoke dùng adapter synthetic ngoài repo xác nhận4 mục/menu/tool panel đã mất; Ctrl+K không bị bắt, Ctrl+F focus query; thêm file không Start, Enter chỉ Start1 lần; Inspector mở/đóng; layout1280×840/800×600 không tràn ngang. Connections/+ gọi thẳng open_connection_editor, form autofocus hostname với nút Connect; Test chỉ gọi test_connection, lỗi Connect giữ hostname và không đóng form, command Connect được nhận mới gọi close_connection_editor. Saved profile chỉ điền form, consent mặc định false, pending khóa input/chống submit trùng và Cancel không Connect/Save. Form600×560/500×480 không tràn ngang, chiều dọc cuộn trong form. Không thêm automated tests; chưa kiểm chứng lại SSH trên host thật hoặc native window lifecycle trong đợt này.
