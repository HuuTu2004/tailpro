# Phase 1 — Evidence triển khai

> Historical validation record. Synthetic examples, fixtures, raw observation artifacts and the `manual-fixtures` feature were removed during source cleanup on 25/11/2026. Commands mentioning them describe past runs and are no longer current setup instructions. Use `npm run contracts` for TypeScript generation and the desktop runtime for real sources.

Ngày 20/11/2026, Asia/Bangkok. Không viết automated tests/framework. Các examples được operator gọi thủ công, in observations và không tự assertion/pass-fail.

## Môi trường

Ubuntu 26.04 x86_64, i5-12400 (12 logical CPUs), RAM 30 GiB, swap 8 GiB; GUI Wayland/X11 có sẵn. Rust/Cargo 1.97.1, Node 24.20.0, npm 11.19.0. Tauri 2.12.7, React 19, Vite 7 được pin trong lockfiles. Linux WebKitGTK runtime 2.52.6 đã có; dev packages chưa cài, sudo cần interactive authentication. Đã download/extract dev packages vào `/tmp/tailpro-build-deps`, chỉnh pkg-config libdir/symlinks trong thư mục tạm và build với PKG_CONFIG_PATH đó; không sửa system libraries.

Ubuntu 24.04 build base chưa kiểm chứng. Hai disposable sshd endpoint chỉ listen loopback, dùng key/log giả trong `/tmp`, không sửa SSH config/known_hosts thật. Docker daemon không có; Docker CLI stub trên endpoint thứ hai chỉ kiểm tra transport/framing/replay classification, không chứng minh Docker compatibility.

## Checks

- `cargo fmt --all --check`.
- `cargo check -p log-core --lib`, `cargo check -p app-core --lib`.
- `cargo clippy -p log-core -p app-core --lib --examples -- -D warnings`.
- Desktop scoped clippy với `-D warnings` và custom-protocol.
- Rust DTO export, `npm run typecheck`, `npm run build`.
- Desktop compile/link với `cargo build -p tailpro-desktop --features custom-protocol --locked` và dev packages tạm.
- Native Tauri window mở trên X11, gọi status/health không có IPC error, đóng qua WM_DELETE_WINDOW: exit code 0. Screenshot chỉ cửa sổ app đã được xem tại `/tmp/tailpro-desktop-final.png`.

Cold core build trong `target/cold-core` riêng: 6.67 s, dependency registry cache warm, dev profile/12 default jobs. Đây không phải cold desktop measurement. Các desktop attempts đầu tìm ra missing icon và missing linker paths; lần link thành công sau chuẩn bị dependencies là 2.33 s với dependency cache warm. Không coi các lần failed build là cold desktop pass. Final desktop rebuild 1,64 s; no-change warm desktop build 0,16 s cùng flags/dependencies.

## Manual acceptance

| Scope | Observation |
| --- | --- |
| SSH static alias | Connect thành công; systemd ProxyCommand của host không liên quan không chặn alias tĩnh |
| Trust | Unknown/changed host key về NeedsAction, không accept/xóa known_hosts; config thay đổi trong soak invalidated master và reconnect |
| 2 endpoints / 4 file sources | Cả bốn streaming sau 30 s ngừng consumer; warm descriptor-open 43/34/33/38 ms, shutdown 4 ms |
| File quoting | Path chứa quote và space mở/đọc được qua template |
| Rename rotation | Có Log rotated và record từ file mới |
| Copytruncate/regrow | Có Log truncated và record mới; continuity uncertain |
| 5 GiB sparse file | Thấy dòng cuối qua recent-window seek; không quét từ đầu |
| Normalization | JSON-unescaped ANSI bị strip; malformed/depth 80 có flag; frame 1 MiB giữ 256 KiB + truncated và raw end offset đúng |
| Multiline | Python traceback giữ cùng error group, INFO header kế tiếp độc lập; fixtures Go/Node được cung cấp để tiếp tục acceptance |
| Docker CLI stub | Giữ đủ 20 stdout/stderr records có cùng timestamp/nội dung; suspected replay không bị suppress |
| Delta overflow | Capture trước 100k/500k append; catch-up trả complete=false, journal chỉ giữ 16.384 changes |
| Pinned excess | Snapshot pin ở 166k: resident ~319 MB; pressure loại input có gap/counter, ba quiet sources giữ đủ 67 records mỗi nguồn; release pin giảm resident còn ~17 MB |

Warm-open chỉ bốn mẫu trên loopback với keys có sẵn, không phải p95/VPS/cold-connect guarantee. “2 endpoints” là hai sshd processes trên cùng máy, không phải hai VPS độc lập. Stop/shutdown chỉ kill/reap owned SSH children.

## Profiling core

Driver `cargo run -p log-core --example profile -- <count> [pace] [pin]`, dev profile, custom counting allocator trong example; payload JSON 499 bytes, không phải fixture 512-byte của release target. Counter allocations gồm driver/parser/store; allocator instrumentation làm chậm và không dùng để suy release throughput.

| Input hot | Quiet mỗi nguồn | Ingest wall | Live rows | Charged bytes | Peak RSS | Recent 50 |
| --- | --- | --- | --- | --- | --- | --- |
| 100.000 | 14 | 4,109 s | 100.042 | 186.928.565 | 150.348 KiB | 159 µs |
| 500.000 | 67 | 20,439 s | 170.634 | 318.899.799 | 252.156 KiB | 444 µs |

500k run: accepted 500.201, discarded 0, evicted 329.567. Byte budget chạm trước row cap; không claim retain đủ 500k. Allocation calls ~2,02M / 9,72M, peak allocated ~132,1 MB / 223,5 MB; khác RSS vì allocator/runtime/mapping overhead. Recent times là single samples, không phải p95. Chưa có release profile measurements.

## SSH file soak

Generator ghi 600.000 synthetic records trong 600,0 s, 1.000 records mỗi giây. Headless application reader + engine + preview mỗi khoảng 1 s; runtime thực dài hơn vì driver cũ lặp 610 samples cộng query time. Có một config/trust-generation change khi chuẩn bị endpoint thứ hai; recovery có marker, do đó đây là soak có reconnect.

- Peak RSS cây tiến trình local: 234,74 MiB, tối đa 3 processes (app + master + channel); 719 samples/~730 s.
- Cuối run: retained 272.296, charged 298.981.008 bytes, discarded 704, shutdown 0 ms.
- Store pressure khi preview cũ scan nhiều rows và pin segments đã tạo explicit gaps. Sau quan sát đã đổi preview sang bounded per-source suffix, core recent-50 đo ở trên. Chưa rerun toàn soak 10 phút sau thay đổi này; không claim sustained zero-drop pass.
- Remote helper có một process/source tại thời điểm kiểm tra. CPU counter 0,07 s chỉ theo helper ban đầu trước reconnect, không phải measurement đầy đủ cho toàn soak.

## Gate chưa đóng

Docker thật (restart/recreate/downtime/permissions), ProxyJump/agent socket/MaxSessions/network outage trên target thực; FIFO race/permission revoke/guard edge cases đầy đủ; config provider fixtures/large-directory/denied/cancel matrix; quiet-source rate scenario 15k/s + 3×2/s thật; release/GUI RSS and freeze; Ubuntu 24.04 build/runtime matrix; cold desktop và edit-parser/edit-DTO timings đầy đủ.

Codex CLI 0.170.0 và Claude Code 2.1.304 có sẵn, nhưng chưa spike MCP handshake hai clients hoặc pin SDK/protocol từ compatibility evidence. Không thay global client config hay tạo MCP crate trước phase 3. Các mục này phải hoàn tất trước khi chốt gate/estimate; implementation hiện chưa đủ để công bố phase 1 fully accepted.

Disposable sshd processes đã được stop sau acceptance; generated private/public credentials và authorized_keys của fixture đã xóa. Không để fixture daemon chạy nền.
