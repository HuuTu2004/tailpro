# Phase 1 — Core, SSH và nguồn log

**Mục tiêu:** Có lõi bounded chạy được, SSH an toàn và nguồn Docker/file có discovery/resume. Đây là nền cho UI và MCP.

**Điều kiện bắt đầu:** Không có; bắt đầu từ xác nhận môi trường ở 1.1.

Khi giao code, chỉ định một mục, ví dụ `1.1`. Mục khác trong phase là kế hoạch tiếp theo, không tự triển khai cùng. Trạng thái thực nằm ở [process](../process/current-status.md), thứ tự tổng ở [roadmap](roadmap.md).

## 1.1 — Desktop shell và workspace

**Đầu ra:** Mở được ứng dụng Linux với hai core library độc lập và vòng lặp compile nhỏ.

**Dependency:** Không có · **Code:** `Cargo.toml`, `crates/{log-core,app-core}`, `src/app/`, `src-tauri/`, DTO generation.

1. Xác nhận CPU/RAM/GUI/toolchain; chuẩn bị build base Ubuntu 24.04, runtime 24.04/26.04 và disposable SSH/Docker target với dữ liệu giả.
2. Scaffold Tauri/React, hai library theo dependency map; window gọi health use case qua typed DTO.
3. Thiết lập dev profile, bindings chỉ ghi khi thay đổi và scoped check/build theo compile guide.

**Xong khi:** Desktop mở/đóng sạch; core build headless, Rust/frontend checks phù hợp chạy được; ghi môi trường và cold/warm build thật.

**Giới hạn:** Chỉ nền tảng chạy app; SSH/log/MCP thuộc các mục tiếp theo.

## 1.2 — Kết nối SSH và tái sử dụng phiên

**Đầu ra:** Chọn host có sẵn hoặc nhập profile rồi kết nối an toàn, dùng chung một master cho các nguồn trên host.

**Dependency:** 1.1 · **Code:** `app-core/adapters/ssh/`, connection use cases, `src/features/connections/`, settings adapter.

1. Import tĩnh alias/Include trong caps; hiển thị các phần dynamic cần consent thay vì tự execute config.
2. Dùng OpenSSH với ProxyJump, SSH Agent/IdentityAgent đã cấu hình; hỗ trợ 1Password hoặc key manager qua agent socket sẵn có, không cài/tạo daemon.
3. Triển khai host-key/trust generation, auth states và ControlMaster thuộc app; invalidation khi config thay đổi.
4. Thêm connect/disconnect, keepalive/backoff/NeedsAction, cancellation và kill/reap đúng owned children.

**Xong khi:** Thử alias, jump, agent socket, unknown/changed host key, config đổi và mất mạng; nhiều nguồn reuse master, stop không reconnect lại. Đo latency warm channel, không suy từ cold connect.

**Giới hạn:** Warm open <50 ms là target cần đo; vẫn tôn trọng MaxSessions/MaxStartups và remote limits. Keychain native macOS cần platform gate trong backlog sau 5 phase, không hứa 100% mọi config.

## 1.3 — Chuẩn hóa JSON, ANSI và stack trace

**Đầu ra:** Biến physical records thành event đọc được, giữ đủ stack khi lọc lỗi.

**Dependency:** 1.1 · **Code:** `log-core/{model,parse}/`, synthetic `fixtures/`.

1. Thêm bounded framing, timestamp/level/message/ID/duration extraction và opaque wire IDs.
2. Strip ANSI trước parse và sau JSON-unescape; malformed/oversize input trả text/quality flags có giới hạn.
3. Nhận dạng Go panic, Python traceback, Node error; nối continuation cùng source/stream/generation, flush theo caps/idle/stop.
4. Giữ raw bounded cho Inspector, escaped khi render; phân biệt event_time và observed_at.

**Xong khi:** Thao tác với JSON/text/ANSI escaped, malformed/deep/large JSON và ba kiểu stack; level:error giữ các phần stack còn retained, không nuốt header kế tiếp. Đo allocations với input lớn.

**Giới hạn:** Chỉ heuristic stack cơ bản; custom regex/multiline profiles ở backlog sau 5 phase.

## 1.4 — Bộ đệm log có giới hạn và công bằng nguồn

**Đầu ra:** Giữ event trong RAM với retention rõ ràng, nguồn ồn không đẩy sạch log của nguồn ít log.

**Dependency:** 1.3 · **Code:** `log-core/{engine.rs,store}/`, `app-core/workers/`, bounded ingest scheduler.

1. Triển khai single writer, charged segments/retired pins và per-source byte/row reservations theo architecture.
2. Thêm fair queues/DRR, dispositions và contiguous ACK interface có generation fence cho source readers.
3. Capture active tails/snapshot/subscription atomically; delta catch-up tới fence, overflow trả incomplete.
4. Cấp bounded query leases/jobs và release/cancel cooperative; expose counters đúng nghĩa.

**Xong khi:** Đo 100k/500k event và actual process-tree RSS; hot 15k/s + ba quiet 2/s vẫn giữ reserved suffix. Thử pinned excess, segment straddle, quiet burst, snapshot race và consumer ngừng đọc 30s; mọi drop có gap. Kiểm chứng UI thật tiếp ở phase 2.

**Giới hạn:** Áp dụng ngân sách chuẩn, không thêm index/disk spill hoặc clone toàn ring để đạt demo.

## 1.5 — Đọc Docker logs và resume

**Đầu ra:** Start/stop và theo dõi log container qua SSH, reconnect có checkpoint RAM.

**Dependency:** 1.2, 1.3, 1.4 · **Code:** `app-core/adapters/sources/` Docker reader, source registry/use cases.

1. Dùng typed container IDs, Docker timestamps/follow và source health; stdout/stderr giữ stream identity.
2. Lưu committed RFC3339Nano cursor sau contiguous ACK; reconnect --since, ledger chỉ phân loại replay.
3. Giữ cursor khi restart cùng ID; ID mới tạo source generation mới, gap/uncertain đúng ngữ nghĩa.
4. Áp dụng per-source working/queue caps, cancellation và bounded remote processes.

**Xong khi:** Thử cùng timestamp, disconnect khi đang assembly, downtime nhiều log, restart/recreate và quyền Docker thiếu. Không --tail giới hạn làm cắt replay, không hard-suppress dòng hợp lệ trùng nội dung.

**Giới hạn:** Compose grouping metadata thuộc 1.7; tự bám container recreate phức tạp ở backlog sau 5 phase.

## 1.6 — Đọc file log, rotation và byte resume

**Đầu ra:** Nhập một đường dẫn, đọc file qua SSH và phục hồi vị trí tốt nhất có thể.

**Dependency:** 1.2, 1.3, 1.4 · **Code:** `app-core/adapters/sources/` file helper/reader/checkpoint.

1. Chứng minh helper template chỉ đọc: descriptor-first open/type handshake, seek/chunks có framing và bounded idle polling; không cài lên VPS.
2. Lưu device/inode/generation/next raw-byte offset sau contiguous ACK; resume đúng offset+1 trước decode.
3. Initial recent window resync newline; rotation phát marker rồi đọc file mới từ 0, truncate/guard mismatch reset có uncertainty.
4. Recheck identity/quyền/regular type; hủy helper và reap khi stop/disconnect.

**Xong khi:** Thử file multi-GiB, tên có quote/space, partial record, rename rotation, copytruncate/regrow và FIFO race. Đo remote CPU/process count và seek latency; không claim exactly-once khi bytes đã mất.

**Giới hạn:** Chỉ file regular seekable; discovery không được tự mở nội dung để hỗ trợ feature này.

## 1.7 — Tìm nguồn log theo ba tầng

**Đầu ra:** Bấm Discover hoặc gõ đường dẫn để chọn được nguồn có provenance và quyền rõ ràng.

**Dependency:** 1.2; Start source dùng 1.5/1.6 · **Code:** `app-core/adapters/discovery/`, `src/features/sources/`, bounded settings/pins.

1. Tầng 1: Docker/Compose metadata, bounded running-unit listing, selected PID FD hints và config providers Nginx/Caddy/Supervisor/Systemd có opt-in; PM2 directory metadata không gọi CLI.
2. Tầng 2: /var/log depth 2 và project presets do user chọn; xét regular files trong caps, extension chỉ ưu tiên, không lọc cứng bỏ file không đuôi.
3. Tầng 3: literal-prefix autocomplete, debounce/latest-wins, tags và pinned successful paths per host.
4. Hiển thị Ready/Denied/Unknown/partial cùng provenance; mtime là Recently modified, Active chỉ khi có bằng chứng metadata tăng kích thước trong cửa sổ quan sát.

**Xong khi:** Discover không đọc log content; config cần consent riêng. Thử roots/includes/symlink, directory lớn, cancel/timeout, denied và unsupported providers; caps chặn trước enumeration. Start recheck descriptor, pins không auto-stream.

**Giới hạn:** Không find /, lsof +D, đọc env hàng loạt hay auto-start PM2 daemon; journal hints dùng 4.1 khi adapter sẵn sàng.

## 1.8 — Read-only mode và nhãn Production

**Đầu ra:** Host Production nổi bật và mọi remote operation đi qua read-only capability policy.

**Dependency:** 1.2, 1.7 · **Code:** `app-core/application/` operation policy, ssh templates, connections/sources UI.

1. Dùng allowlist typed remote operations, bundled UI capabilities tối thiểu; không có arbitrary command input/API.
2. User gắn environment label Production cho host; viền/badge nhất quán trên picker/table/detail và accessibility label.
3. Giải thích riêng quyền đọc log/config/topology và grant external agent; label không tự Start/cấp quyền hoặc sửa SSH config.

**Xong khi:** Kiểm tra mọi remote template chỉ phục vụ read/probe/follow, không sudo/install/write/service control; nhãn Production hiện trên connection/source views đã có. Application API không nhận arbitrary command text; bảng/Inspector/MCP mới ở phase 2–3 phải reuse policy và hiển thị nhãn tương ứng.

**Giới hạn:** Read-only nói về hoạt động TailPro trên VPS; SSH có thể tạo audit log server và TailPro vẫn lưu settings cục bộ.

## Gate kết thúc phase

Core/desktop build được; chọn nguồn, ingest và stop sạch trên 2 host/4 sources. Checkpoint/gaps đúng và quiet sources được bảo vệ. Chứng minh helper byte-seek/remote overhead, discovery không đọc log content; ghi baseline generator 1k events/s 10 phút, freeze consumer 30s. Trước khi chốt estimate, spike giới hạn handshake hai MCP clients cho mục 3.3; không tạo toàn bộ SDK/crate tương lai.

## Tài liệu chuẩn khi code

Đọc [architecture](../system-design/architecture.md), các mục liên quan trong [contracts](../system-design/contracts.md) và [compile guide](build-performance.md). Budget/behavior theo system-design; plan không thay đổi chúng. Acceptance dùng build/lint/typecheck/manual smoke/profiling; không viết automated tests.

## Điểm vào implementation

- Chạy desktop từ workspace root: `npm ci`, `npm run contracts`, `npm run tauri dev`.
- Scoped checks theo [compile guide](build-performance.md); thao tác smoke/profiling và số đo ở [acceptance](../process/phase-01-acceptance.md).
- Hai core libraries build headless qua default workspace members; desktop package là `tailpro-desktop`, embedded frontend dùng feature `custom-protocol`.
- `app-core/examples/session.rs` chọn disposable SSH alias và synthetic file/container; `multi_source.rs` chạy hai endpoint/bốn file. Đây là driver thủ công, không có automated assertions/tests.
- Module tồn tại hoặc build pass chưa đủ đóng gate; trạng thái chính thức nằm ở process.
