# Phase 3 — Evidence triển khai

> Historical validation record. Synthetic examples, fixtures, raw observation artifacts and the `manual-fixtures` feature were removed during source cleanup on 25/11/2026. Commands mentioning them describe past runs and are no longer current setup instructions. Use `npm run contracts` for TypeScript generation and the desktop runtime for real sources.

Ngày 20/11/2026, Asia/Bangkok. Triển khai code 3.1–3.6 theo yêu cầu toàn phase. **Gate PARTIAL**: các prerequisite runtime của phase 1–2 vẫn còn, Claude mới handshake/discover, chưa chạy model Ollama/BYOK thật. Không viết automated tests/framework; examples in observations để operator đọc thủ công.

## Code và giới hạn hiện hành

- `app-protocol`: desktop DTO được extract khỏi app-core, DTO MCP/output riêng không chứa global sequence/event ID; TypeScript và read JSON Schema generate từ Rust. `log-core` vẫn không có Tokio/Tauri/SDK.
- `application/redaction.rs`: JSON-unescape trước recursive projection; sensitive keys, query parameters, userinfo, Bearer, compact JWT/JWE. Input tối đa 64 KiB, depth 32/4096 nodes, percent-decode tối đa hai lượt. Candidate có encoding được mask rộng để không nhầm encoded separators thành public parameters; malformed encoding, ambiguous embedded JSON/backslash và oversize được mask với `redaction_incomplete`. Policy cố ý conservative, có thể mask thêm text/hostname; không hứa phát hiện mọi secret tự do.
- Predicate, exported row, MCP context và Explain Error dùng projection từ sanitized raw, không dùng extracted fields của raw gốc. Nested sensitive object không rò `level/time/trace_id` qua normalization. Không fetch URL trong log hoặc log secret/hash của secret.
- Native desktop principal là bundled `main` window trong app session; export/AI preparation thuộc principal này, không expose qua MCP. Prepared selection/snapshot/profile nằm trong exact payload, handle/digest/epoch/TTL được giữ phía runtime. Revoke/expiry grant, clear/source stop/disconnect/environment/provider change invalidate preparations; commit rechecks epoch/TTL ngay trước atomic publication.
- Export cap alpha **2 MiB/10.000 events**, nhỏ hơn ceiling 8 MiB để giữ chỗ cho UI cache/preview; một prepared export, TTL 60 s. NDJSON metadata record và CSV metadata columns độc lập sidecar; quote mọi cell, neutralize leading formula/control (cả sau whitespace). Native path là absolute destination nhập rõ; private temp 0600, fsync, atomic rename khi overwrite được chọn hoặc atomic no-clobber hard link, cleanup failure/cancel. Không tự gửi file lên dịch vụ.
- Một Unix bridge dùng chính AppHandle/engine đang chạy. Directory 0700, socket/registry/lock 0600, O_NOFOLLOW/O_NONBLOCK/CLOEXEC, flock giữ bằng fd, backlog 8, 8 accepted sockets, 2 preauth, auth 200 ms, authenticated idle 60 s. Session socket mới mỗi lần chạy; GC chỉ refused socket có cùng UID/inode/type, dưới exclusive lock. Adapter không cleanup/start engine/SSH.
- SDK `rmcp = 0.8.5` được pin exact trong manifest/lock, MCP negotiated **2025-06-18**; SDK ở executable `tailpro-mcp`. Stdio frames 64 KiB; tối đa hai outstanding protocol requests trước SDK dispatch, không dùng codec mặc định vô hạn. Private bridge request ≤64 KiB, MCP output mặc định 16 KiB/50 events; alpha bridge share tối đa **256 KiB** (nằm dưới hard ceiling 1 MiB), ≤500 events. App query lane một background job dùng chung CPU/background permits, chừa interactive lane. Rate 10 tool calls/phút/grant, ≤2 concurrent/grant; adapter reject overlapping reads thay vì queue.
- Enrollment tối đa 8 clients; token/session/grant chỉ RAM, grant TTL 1 giờ. UI thông báo external client/model có thể nhận log, sources/Production labels, explicit consent và revoke. Bridge rechecks grant trong từng nonblocking send chunk; revoked sockets không gửi tiếp.
- Observation baseline `get_recent_logs(limit=0)` không pin retention. Forward pages giữ fence, chỉ advance tới last returned/scanned match; byte/count cap giữ matching row chưa trả cho lần sau. Recent backward page cursor khác observation cursor; page TTL 30 s, observation TTL 10 phút. Query/scope/generation/grant mismatch reject, single-flight, failure/cancel không advance. Input handle giữ immutable cạnh returned continuation để mất socket trước khi nhận response vẫn retry được; TTL/resource eviction vẫn có thể yêu cầu requery. Tối đa 2 handles/client, 16 global và 64 KiB cursor metadata; idle handles có thể bị thay thế, trả expired/requery.
- Evidence refs opaque, bound grant/source generation, TTL 10 phút; registry alpha 1536 entries trong ceiling 4096/1 MiB. Context hiện gồm reference và bounded **preceding** rows cùng nguồn; không dùng khoảng cách global sequence để chọn context. Source generation đổi khi file opened/rotated/truncated. Gaps có nhãn và sticky trong observation chain; capture baseline mới để bắt đầu chain khác. Không có field kết luận verified fix/deployed.
- Explain Error tắt mặc định, provider setting chỉ app session. Local chỉ literal `127.0.0.1`/`[::1]` HTTP `/api/chat`, no proxy/redirect, kiểm tra locally installed metadata qua `/api/tags` và `/api/show` sau consent và reject remote model metadata/`:cloud`. BYOK OpenAI-compatible endpoint phải HTTPS, credentials chỉ OS Secret Service qua `secret-tool`, không plaintext fallback. Preview bind exact endpoint/model/digest/epoch; one-shot consent, deadline tối đa 30 s hoặc remaining prepared TTL, request/response ≤64 KiB, ≤100 events, 1 provider request global; cancel/config/revoke đóng future/I/O. Selected IDs là evidence, model text nằm riêng trong hypotheses/untrusted panel, React text rendering, không action execution hay automatic cloud fallback.

Các caps là giới hạn logical implementation; chưa coi RSS, allocator/SDK transient allocation và worst-case shared 8 MiB bridge accounting là được certification bằng số đo. Không thay đổi ngân sách chuẩn trong architecture/contracts.

## Commands đã chạy

```bash
cargo fmt --check
cargo clippy -p app-core -p app-protocol -p tailpro-mcp -p log-core \
  --lib --bins --examples --features app-core/manual-fixtures -- -D warnings
cargo build -p tailpro-mcp
cargo run -p app-core --example redaction_canaries
npm run contracts
npm run build
PKG_CONFIG_PATH=/tmp/tailpro-build-deps/root/usr/lib/x86_64-linux-gnu/pkgconfig \
  cargo clippy -p tailpro-desktop --features custom-protocol -- -D warnings
PKG_CONFIG_PATH=/tmp/tailpro-build-deps/root/usr/lib/x86_64-linux-gnu/pkgconfig \
  cargo build -p tailpro-desktop --features custom-protocol --locked
```

Native dependencies là packages đã extract ở phase trước; không sửa system libraries. `manual-fixtures` không nằm trong default features/desktop, không có fixture command trên wire. Nó chỉ cho manually invoked examples seed synthetic sources vào cùng runtime để quan sát policy, không thay SSH/source acceptance.

```bash
# XDG_RUNTIME_DIR được operator tạo trước, owner-only 0700.
XDG_RUNTIME_DIR=/tmp/tp3-client-runtime target/debug/examples/security_observations /tmp/tp3-observation-final hold
XDG_RUNTIME_DIR=/tmp/tp3-evict-runtime target/debug/examples/security_observations /tmp/tp3-observation-evict evict
XDG_RUNTIME_DIR=/tmp/tp3-expiry-runtime target/debug/examples/security_observations /tmp/tp3-observation-expiry expiry
# Build helpers: cargo build -p app-core --examples --features manual-fixtures
```

## Observations

| Hạng mục | Evidence local |
| --- | --- |
| Canary | Nested JSON, duplicate/encoded token URL, postgres userinfo, JSON-unescaped Bearer, JWT/JWE không còn canary; malformed percent và huge candidate báo incomplete. Embedded JSON/Basic auth cũng được mask |
| Projection equivalence | Hai nested sensitive objects khác secret cho cùng raw projection/predicate; trace/time từ sensitive parent đều absent. Search `CANARY_SECRET` trả 0 |
| Baseline/paging | Capture baseline rồi thêm 7 matching records; pages 3/3/1 trả đúng row 9–15. Recent page 7–8 rồi backward 5–6 không overlap. Không expose ingest sequence |
| Cursor cancellation/rate | Đóng socket sau khi gửi page rồi retry input handle vẫn trả rows 12–13 đúng boundary; đổi query bị reject. Byte cap 2048 B giữ next page; sau các read trước, 7 list_sources thành công rồi 5 calls bị rate-limit |
| Authorization | Ungranted source bị reject; grant revoke reject read và invalidate prepared export. Native consent UI chặn grant trước checkbox |
| Prepared bytes | NDJSON bytes trên disk bằng preview, không có CANARY. Sau 10.000×32 KiB synthetic appends, selected event thực sự expired khỏi Inspector nhưng commit vẫn bằng owned preview |
| CSV | Normal/expiry driver có multiline formula cell được prefix apostrophe và JSON quotes được double; metadata cùng mỗi row. Eviction driver không còn selected rows nên CSV chỉ metadata, ghi incomplete |
| Failure/expiry | Cancel, missing destination directory, revoked preparation và preview sau 61 s bị reject; không tạo file cancel/revoked/expired. No-clobber là atomic operation, không check-then-overwrite |
| Crash lifecycle | SIGKILL fixture owner để lại socket; restart đổi session, dọn orphan refused, còn một socket. `instance.lock` inode 354 không đổi. Old-session adapter exit 1 trong khoảng 1 ms, stdout 0 bytes |
| Bridge limits | Dir/socket 0700/0600. Owner lock/socket descriptors có CLOEXEC. Slow preauth sau 250 ms và 65.537-byte frame bị đóng |
| Raw MCP client | Initialize offer 2025-11-25 negotiate 2025-06-18, discover đúng bốn tools, search 2 sanitized events và context 3 rows; stdout chỉ JSON-RPC, exit 0 |
| Codex CLI 0.170.0 | App-server ephemeral thread, `mcpServerStatus/list` connected với đủ 4 tools; `mcpServer/tool/call` search 2 rows không canary, get_log_context 3 rows. Không tạo model turn |
| Claude Code 2.1.304 | `claude mcp list` trong private `CLAUDE_CONFIG_DIR` báo Connected với adapter thật. Chưa gọi domain tools/observation qua Claude model |
| Explain HTTP fixture | Không provider call lúc prepare/consent=false. Sau consent có `/api/tags` + `/api/show` + `/api/chat`, không canary, thành công dù proxy env trỏ port đóng. Redirect/404 model/remote metadata chặn trước chat; DNS-local và HTTP-cloud config bị reject; cancel trong lúc chat trả Cancelled và fixture thấy peer đóng I/O |
| Secret store | Máy không có `secret-tool`: `OS Secret Service unavailable; BYOK disabled`; không ghi plaintext key hoặc fallback cloud |
| Browser fixture | Grant disable trước consent, enable sau consent; revoke xóa grant/secret panel. Export render exact preview. Explain send disable trước consent, 0 → 1 request sau consent; `<script>` trong preview/model output hiển thị literal, không chạy, không window errors |

CLI configurations/enrollments chỉ tạo trong private `/tmp` fixture; không sửa global Codex/Claude config, không gọi model trả phí hoặc gửi log thật. Browser evidence dùng IPC fixture, không thay loaded native integration. HTTP fixture chỉ chứng minh transport/policy; không được gọi là local model inference thật.

Native custom-protocol binary của snapshot phase 3 đã mở cửa sổ X11 với config/runtime riêng, tạo private bridge, đóng qua WM_DELETE_WINDOW exit 0; socket/registry được dọn, lock file còn nguyên. Đây là empty-workspace bootstrap/shutdown smoke.

Các checks pass ở trên thuộc snapshot phase 3. Trong lượt verify cuối, workspace bắt đầu có thay đổi phase 4 đồng thời (`production`/`topology`, Journal/correlation DTOs); lần `cargo fmt --check` khi các module còn đang được ghi đã không pass. Giữ nguyên thay đổi song song, không dùng evidence snapshot phase 3 để xác nhận build của phase 4 hoặc workspace đang thay đổi.

## Gate còn thiếu

- Claude tool calls và observation cùng incident qua client thực; full client version/runtime matrix.
- Ollama/model local thật, BYOK HTTPS provider + OS Secret Service thật; network cancellation/revoke/expiry race matrix đầy đủ.
- Revoke trong lúc blocked send, page cancellation/single-flight/source-generation/retention-gap stress, rate-limit/concurrency/socket backlog/permission/FIFO/path races và shared-memory/RSS profiling dưới tải.
- Native loaded UI/SSH/export/clipboard integration; Ubuntu 24.04/26.04 distribution và các gate phase 1–2 cũ.

Không công bố full phase acceptance, agent-ready release hoặc performance/compatibility certification từ các observations này. SDK/protocol reference: [rmcp 0.8.5](https://docs.rs/rmcp/0.8.5/rmcp/), [MCP 2025-06-18 lifecycle](https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle), [Ollama API model metadata](https://github.com/ollama/ollama/blob/main/api/types.go).
