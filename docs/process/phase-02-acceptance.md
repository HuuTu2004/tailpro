# Phase 2 — Evidence triển khai

> Historical validation record. Synthetic examples, fixtures, raw observation artifacts and the `manual-fixtures` feature were removed during source cleanup on 25/11/2026. Commands mentioning them describe past runs and are no longer current setup instructions. Use `npm run contracts` for TypeScript generation and the desktop runtime for real sources.

Ngày 20/11/2026, Asia/Bangkok. Scope được giao là toàn file `phase-02-viewer-experience.md` (2.1–2.5). Không tạo automated tests/framework; các examples chỉ in observations, được gọi thủ công.

## Implementation

- `log-core/query`: compile ≤4 KiB/32 clauses; literal/AND/field/exact IDs/duration ms/s. Scan theo captured snapshot, giữ tối đa 200 references trong heap rồi sắp local ingest order; không tạo index toàn store. Có deadline/cancellation và cursor/anchor expired rõ.
- `application/views`: 4 view handles, idle TTL 60 s, page/detail generations riêng và cooperative cancellation. Hai CPU permits dùng chung với normalization; background normalization chỉ chiếm tối đa một permit; tối đa 4 query/detail waiters trước spawn. Snapshot chỉ sống trong job, không pin retention suốt thời gian pause.
- Owner capture cài subscription cùng fence; frontend đọc metadata mỗi 100 ms và ACK notice trước. Một pending notice/view; không push/queue từng dòng. Journal overflow trả incomplete, UI đọc snapshot hiện tại và giữ cảnh báo coverage.
- DTO schema 1 generate từ Rust; page ≤200 rows/512 KiB exact serialized bytes. Message preview ≤2048 chars, trace preview ≤256 chars; Inspector đọc raw riêng. JS cache ≤10 pages/2000 rows/24 MiB logical charge, để phần còn lại của budget 32 MiB cho Inspector/metadata/UI. Charge JS là estimate UTF-16 + overhead, không phải hard WebView heap cap.
- Inspector borrow RawValue để chọn từng nhánh, ≤200 children/expand, path depth 64; không eager-tree toàn raw/store. Client giữ ≤4096 nodes/1 MiB conservative logical charge; job/detail IPC ≤512 KiB, control-heavy raw có fallback preview ghi rõ truncation. Không giữ detail snapshot sau trả DTO; close/change selection hủy job cũ. Copy UI cục bộ có nút rõ, preview đã truncate không giả làm full value.
- Pause giữ page DTOs và opaque viewport event ID; đọc older bằng retained event cursor, recheck anchor khi scroll/retention đổi, expired có lựa chọn reload retained/jump live. Arrivals tính từ accepted notice watermarks của tất cả nguồn, độc lập query; không queue arrivals khi pause.
- Saved Views ≤32, name ≤80 bytes, query/scope được validate, lưu trong settings version 1 với field optional để đọc file cũ. Restore chỉ thay query/source IDs, không connect/discover/start/grant.
- Palette reuse bounded completion/start API; chọn host không connect, chọn file không Start. Ctrl+K/Ctrl+F, Escape, viewport Space/arrows/Enter và focus restore. Production, source health, discarded/evicted/gaps nằm ngoài query rows.

## Checks

```bash
cargo fmt --all --check
cargo check -p log-core -p app-core --lib
cargo clippy -p log-core -p app-core --lib --examples -- -D warnings
cargo run -p app-core --example export_contracts
npm run typecheck
npm run build
cargo clippy -p tailpro-desktop --features custom-protocol -- -D warnings
cargo build -p tailpro-desktop --features custom-protocol --locked
```

Các check/build đã chạy qua. Native commands dùng `PKG_CONFIG_PATH=/tmp/tailpro-build-deps/root/usr/lib/x86_64-linux-gnu/pkgconfig` với dev packages extract từ phase 1, không cài/sửa system libraries. Native smoke có ghi chú riêng bên dưới.

## Core query observations

Ubuntu 26.04 x86_64, i5-12400/12 logical CPUs, Rust 1.97.1 dev profile, không allocator instrumentation. Driver:

```bash
cargo run -p log-core --example viewer_profile -- 100000
cargo run -p log-core --example viewer_profile -- 500000
```

Synthetic JSON, message 260 chữ `x`, 4 sources ánh xạ 2 server names; mỗi business ID xuất hiện ở cả bốn nguồn. 25 warm samples/query; số đo không gồm Tauri/WebView. Input 500k chạm byte cap trước row cap.

| Query | p95 / 100k retained | p95 / 237.924 retained sau 500k input |
| --- | --- | --- |
| Latest page / empty query | 11,34 ms | 36,81 ms |
| trace_id exact | 7,87 ms | 25,60 ms |
| order_id AND level:error | 11,52 ms | 25,60 ms |
| Missing user_id | 10,53 ms | 23,18 ms |
| Duration >200ms AND ≤1s | 11,24 ms | 37,34 ms |
| Quoted level:error | 13,66 ms | 34,44 ms |
| Literal substring không match (`x x`) | 70,82 ms | 180,63 ms |

100k: accepted 100.000, discarded 0, evicted 0, charged 155.136.166 B. 500k: accepted 500.000, discarded 0, evicted 262.076, charged 369.057.157 B (trong source budget 352 MiB). Không claim scan đủ 500k retained hoặc release benchmark/fixture 512-byte chuẩn.

Quan sát older cursor không overlap boundary, missing field trả 0 rows, ID query trả đủ bốn sources; cancel scan complete=false và unknown anchor expired=true. OR/NOT/regex/wildcard, unknown fields/level, malformed AND/quote và missing/invalid duration unit có lỗi. JSON 5000 keys chỉ trả 200 children mỗi lần, paging offsets 0/200/4800 và nested path `[42]` trả đúng node.

## Freeze / application observations

```bash
cargo run -p app-core --example view_notice_profile
```

Consumer không poll/ACK trong 30 s; producer synthetic bốn nguồn đặt interval 500 records/100 ms:

- 150.500 accepted/30,03 s, discarded 0, evicted 0, charged 157.535.875 B.
- Request không ACK vẫn nhận pending revision 0. ACK revision 0 mới nhận revision 150500; `complete=false` vì bounded journal overflow. Không biến mất coverage thành complete giả.
- Hai page requests generations 1/2: generation 1 Cancelled, generation 2 trả kết quả.
- Saved View chứa source ID không tồn tại được restore/load; source count vẫn 0.
- Inspector handle expired trả lỗi requery. Close view/shutdown kết thúc driver bình thường.

Đây là headless consumer freeze và metadata backpressure, không phải GUI freeze/RSS certification hoặc SSH/Docker soak.

## Browser smoke

Chrome headless local, Vite, fixture IPC trong `/tmp`; không thêm mocks/harness vào sản phẩm/repo. Fixture sinh từng page, không allocate 200k rows trong JS. Kết quả chỉ chứng minh UI behavior, không thay real Tauri→SSH integration.

- Hai host/bốn sources, Production badges và stable source colors; initial 200 cached rows với 20 DOM rows.
- Load older 12 lần: cache dừng 2000 rows, DOM 29 rows; conservative cache charge ~2214 KiB với fixture này.
- Space ở viewport pause; event ID đang đọc không đổi khi notice tăng thêm 100 arrivals. Space ở query nhập được ký tự và không toggle pause.
- Query `old` cố ý delay 350 ms rồi nhập `new`: rows cuối chỉ có query mới; không có window errors. Hai Saved Views khác tên nhưng cùng query/scope không invalidate request đang chạy vô cớ.
- Click event mở Inspector; `<script>` trong raw/JSON được hiển thị literal, không execute. Escape đóng Inspector và focus về viewport.
- Ctrl+F focus query; Ctrl+K focus palette; Escape đóng palette và focus về query. Đã sửa focus cleanup để chịu React StrictMode.
- Simulate eviction của viewport anchor: báo expired và có Reload retained logs; chọn fallback tải page retained, xóa lỗi.
- Một sample diagnostics: frame interval p95 16,7 ms, page 62.148 serialized bytes, E2E ~16,3 ms **với IPC giả**. Frame sampler gồm idle; không dùng con số này để tuyên bố scroll/native query performance đạt.

## Native smoke

Binary custom-protocol mở trên X11/Xwayland với settings riêng dưới `/tmp`, đóng qua WM_DELETE_WINDOW exit 0. Probe tạm ghi `ready=complete`, `live=true`, `grid=true`, `alerts=[]`; trang `tauri://localhost` tải đúng asset mới và notice/page IPC có kết quả 0 retained, không lỗi.

XGetImage của cửa sổ cha/con trả vùng xám trên môi trường rootless Xwayland này, kể cả khi chờ 8 s/tắt DMA-BUF/compositing trong từng lần chạy. Không dùng ảnh đó làm bằng chứng frontend lỗi. Snapshot lấy trực tiếp từ WebKit cho thấy toàn app/viewer render đúng (`/tmp/tailpro-phase2-webkit-snapshot.png`). GTK/X11 key injection trong driver không tạo DOM key observations, vì vậy keyboard acceptance vẫn dựa trên browser fixture, không claim native keyboard pass.

Đã gỡ `native_probe`, hook/debug logs, direct WebKit dependency và lockfile edge tạm; build lại source sạch với `--locked`. Không ép graphics flags vào sản phẩm, không sửa desktop/system config. Native snapshot chỉ dùng empty workspace, không log thật. Chưa coi đây là native loaded-data/compositor/platform certification.

## Gate còn thiếu

Chưa chạy incident mới end-to-end trên hai SSH host/bốn nguồn thật với viewer; phase 1 còn các prerequisite runtime gates. Chưa có native loaded WebView scroll p95/query E2E distribution, full process-tree RSS, GUI freeze 30 s dưới ingest thật, clipboard/platform matrix, sâu/giant raw và eviction stack-part matrix đầy đủ. Chưa release/Ubuntu 24.04 validation. Đây là viewer nội bộ, phase gate vẫn PARTIAL; không agent-ready trước phase 3.
