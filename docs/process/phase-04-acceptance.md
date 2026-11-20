# Phase 4 — Implementation và local observations

Ngày 20/11/2026, Asia/Bangkok. Scope user giao toàn file: 4.1–4.7. **Gate PARTIAL**, chưa production/release certification; các runtime gates Phase 1–3 vẫn giữ nguyên. Không viết automated tests hoặc test framework. Driver dưới đây chỉ in observations từ dữ liệu synthetic để operator kiểm tra.

## Code và semantics

| Mục | Implementation |
| --- | --- |
| 4.1 | Journal source từ discovered systemd unit, read-only bundled Python/journalctl template; bounded JSON frames, journal stream/priority/time normalization. Cursor RAM-only qua contiguous generation-fenced ACK. Resume kiểm tra record tại cursor; vacuum/mismatch/timeout dùng recent-200 fallback có gap. Restricted/unsupported access có trạng thái; không hứa thấy toàn history. |
| 4.2 | `get_error_clusters`: authorized redacted projection, canonical signature v1 bỏ top-level time/correlation/duration fields, giữ nguyên message/stack còn lại. SHA-256 chỉ là bucket key, so toàn representation để xử lý collision. Default 20/hard 128 groups, tối đa 500 observed error samples, signature ≤4 KiB/256 KiB tổng; representation mất evidence do masking thì bỏ với partial. Group ref mở bounded representative context. |
| 4.3 | Settings source/service/project/aliases → declared repo label/root/revision, không mở repo. `inspect_context` trả ≤32 relative path/line hints từ sanitized stack, reject absolute/traversal/URL/command paths. Revision là declaration unverified; không khẳng định deployed checkout. Save/delete mapping revoke contexts/grants. |
| 4.4 | `locate_service`: literal aliases từ selected source/host, Docker metadata được khai báo khi Start và optional developer mapping; chỉ granted source IDs, ambiguity rõ, không Discover/Start. |
| 4.5 | `trace_id` nhận typed `trace_id`/`order_id`, exact sanitized match; optional window theo local observed ms. Scoped trace refs lưu selector/generations trong cùng TTL/LRU registry, không pin logs. `inspect_context` resolve evidence/group representative hoặc requery trace, kèm neighbors/stack/health/gaps/hints. Tám tools, wire schema 2, required arguments riêng từng tool; bốn baseline tools giữ observation semantics. |
| 4.6 | Trace UI reuse bounded application result, group theo source, explicit duration_ms bars và event-only markers; click mở redacted evidence panel. Vị trí bars không được gọi là causal span start; clock/parent semantics unknown. Một result ≤200 events/128 KiB, cancel/supersede qua view generation/query lane. |
| 4.7 | Separate topology consent cho exact files/approved roots/selected literal endpoint names. Remote descriptor walk no-follow/regular file, 64 KiB/file và 256 KiB tổng, ≤32 files; static JSON/block-YAML subset, không execute/interpolate/includes/.env/runtime env. Chỉ giữ allowlisted hostname match, không trả endpoint value/credentials/query/config raw. ≤256 nodes/512 edges/256 KiB, configured/inferred provenance, mapped retained error counts, không rates. |

Continuity marker `Stream::System + quality.uncertain` ghi engine source-loss metadata: filter không chọn marker vẫn phải thấy gap. UI trace/topology dùng existing view lifecycle/metadata ACK để giữ handles, clear result khi source generation/policy revision hoặc input đổi; không có background graph cache. Mỗi topology request đọc snapshot mới; sau consent, descriptor metadata revision được kiểm tra mỗi 15 giây khi đang giữ graph; đổi identity/size/mtime/ctime hoặc verification unavailable thì invalidate và yêu cầu reread. Đây là bounded metadata polling, không phải content-integrity proof.

## Checks đã chạy

```bash
cargo fmt --all --check
cargo clippy -p app-core -p app-protocol -p tailpro-mcp -p log-core \
  --lib --bins --examples --features app-core/manual-fixtures -- -D warnings
cargo build -p tailpro-mcp
cargo run -p app-core --example export_contracts
cargo run -p app-core --example production_observations \
  --features manual-fixtures -- /tmp/<private-fixture-directory> caps
npm run build
PKG_CONFIG_PATH=/tmp/tailpro-build-deps/root/usr/lib/x86_64-linux-gnu/pkgconfig \
  cargo clippy -p tailpro-desktop --features custom-protocol -- -D warnings
PKG_CONFIG_PATH=/tmp/tailpro-build-deps/root/usr/lib/x86_64-linux-gnu/pkgconfig \
  cargo build -p tailpro-desktop --features custom-protocol --locked
```

Dùng dev/line-tables-only profile và caches đang có, Ubuntu 26.04 x86_64; native development dependencies extract trong `/tmp` từ Phase 1, không cài/sửa system packages. `manual-fixtures` chỉ bật trong example; desktop/bridge không có synthetic-ingestion API. `session <SSH alias> journal:<unit> <seconds>` có thể dùng khi operator đã chọn disposable host/unit.

## Local observations

| Probe | Kết quả quan sát |
| --- | --- |
| Same incident, 3 granted + 1 ungranted source | 24 granted errors, secret-only differences cùng nhóm, khác full stack thành hai nhóm count **18/6**. Ungranted source bị reject và không hiện metadata trong kết quả. |
| Clusters vs raw search, cùng incident | Một clusters call: **3.078 B / ~4,3 ms**, 2 representative events; một raw search call: **14.769 B / ~10,6 ms**, 24 events. Giảm khoảng **79% response bytes** ở fixture này; không phải p95/release benchmark hoặc lời hứa ít calls cho mọi incident. Mở context cần call riêng. |
| Alias ambiguity | `checkout` trả hai candidate IDs và `ambiguous=true`; alias `ungranted` trả 0 candidates trong authorized scope, không tự discovery. |
| Correlation/window | Full three-source trace qua MCP với max_bytes 64 KiB trả 24 events; default 16 KiB dừng 18 events và partial. `order_id` window 1002–1004 trả 9 events/3 sources, có ambiguity. Trace ref inspect qua MCP trả 24 timeline events và safe hints. |
| Hint paths | Valid `src/checkout.ts:42` được trả. Mixed hostile stack chỉ giữ `src/ok.ts:7`, bỏ `../bad.ts`, absolute và URL. Encoded/quoted strings có thể bị shared redactor mask rộng trước extraction; không bypass redaction để tạo hint. Declared revision khác event revision vẫn unverified. |
| Caps | 740 errors: retained-sample cap trả 500 represented, incomplete/truncated. 140 unique signatures trong selected window trả 128 groups/116.535 B, incomplete/truncated và mọi group có nhãn. |
| Filtered gaps | Marker Journal không match error filter vẫn giữ `complete=false` và source gap trong clusters response. |
| Journal ACK | ACK cursor-B trước A không advance; ACK generation cũ bị bỏ; sau ACK contiguous A position thành cursor-B. |
| Journal helper subprocess fixture | Valid cursor resume; vacuumed cursor emits gap rồi bounded fallback; permission stderr emits restricted access và unavailable, không coi empty output là history rỗng. SIGTERM cleanup helper/owned child đã quan sát. Đây là journalctl stub, chưa journal daemon thật. |
| Static Compose | Block depends_on và selected credential-bearing DATABASE_URL tạo configured/inferred api→db; output không có credential/query/env value/raw config. Includes/variables unresolved trả partial; symlink config bị từ chối, không follow. Stat-only revision ban đầu khớp content snapshot, sau sửa file đổi revision và stat-only trả 0 nodes/config content. |
| MCP SDK transport | Adapter thật initialize schema 2 / MCP 2025-06-18, discover đúng 8 schemas với required service/correlation/evidence_ref; call locate/clusters/trace/inspect group và trace thành công, stdout chỉ JSON-RPC, exit 0. |
| Codex CLI 0.170.0 | Ephemeral app-server thread connected, discover đủ 8 tools; trực tiếp call locate_service, get_error_clusters, trace_id và inspect_context thành công, không tạo model turn. |
| Claude Code 2.1.304 | Private CLAUDE_CONFIG_DIR, `claude mcp list` Connected với adapter schema 2. Chưa domain tool calls qua Claude model. |
| Browser IPC fixture | 0 window errors; explicit bars, event markers, evidence panel và two-node topology có configured edge/unknown counts. Form gửi exact files/roots/consent; snapshot/provenance/partial labels có mặt. Freshness poll đầu giữ 2 nodes khi revision khớp; poll tiếp với revision đổi xóa nodes và hiện yêu cầu reread, 0 window errors. Đây là browser fixture, không thay loaded native integration. |

CLI configs/tokens chỉ trong owner-only `/tmp` fixture directories, không sửa global Codex/Claude config, không gọi model trả phí hoặc gửi log thật. Source content, config, endpoint secrets đều synthetic. Các probe lần đầu vào fixture đã hết lifetime không tính là client compatibility failure; rerun với live fixture kết nối được.

## Gate còn thiếu

- Journal daemon/SSH target thực: unit có/không quyền, vacuum thực, disconnect/reconnect, unsupported systemd/Python version, empty/restricted history và remote CPU/process resources.
- Claude specialized domain calls cùng incident; full client cancellation/revoke/TTL/generation/rate/resource race matrix. Reference TTL vẫn 10 phút, không bảo đảm retention.
- Loaded native waterfall/topology với source thật: clock skew, missing parents, partial retention, keyboard/selection/filter/cancel và RSS/process-tree measurement trong shared pools.
- Compose hỗ trợ static subset; imports/includes/merges/interpolation được ghi unresolved, không follow. Runtime endpoint environment provider cố ý unavailable; hiện chỉ allowlisted literal values trong approved config. Freshness dùng metadata revision mỗi 15 giây, không đọc lại config content; metadata không chứng minh immutable config khi hostile filesystem thay đổi.
- Soak/stress cho shared query/bridge ledger, deadline/collision/byte caps và mapping/grant races rộng hơn. Các cap/permits cục bộ và scoped builds không đủ chứng minh 64 MiB/8 MiB/RSS targets toàn runtime.

Chưa gọi phase fully accepted hoặc production-ready. Next action: chạy target/native + Claude domain matrix trên synthetic incident được operator chọn, sau đó đóng runtime/resource gates trước Phase 5 release claims.
