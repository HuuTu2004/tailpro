# Contracts bắt buộc khi triển khai

AI đọc phần liên quan trước khi code. Cấu trúc/ownership/budget chuẩn nằm trong [architecture](architecture.md); acceptance dưới đây được thực hiện thủ công/build/lint/profiling theo AGENTS.md, không viết automated tests/framework mới.

## SSH và quyền truy cập

OpenSSH native, agent hoặc noninteractive key; host checking nghiêm ngặt, không tự accept/xóa known_hosts. Không agent forwarding, sudo, package install, arbitrary exec hoặc remote writes. Remote helper là template chỉ đọc cố định chạy tạm qua SSH, không cài agent.

Import config chỉ đọc tĩnh; không auto `ssh -G` vì Match exec có thể chạy code. Trust generation bao gồm user/system/Include contents+glob membership/jump/overrides/binary/trust mode; cap100 files/1MiB/depth8. Recheck trước resolve/spawn/mux reuse; input đổi invalidate generation/master. Dynamic closure không rõ phải disclose, session-only và không reuse master xuyên reconnect dưới fingerprint giả. Không hứa chống malicious same-UID config races.

Local argv không qua shell; remote POSIX shell arguments phải validate/quote tập trung. Chỉ trusted templates nhận typed paths/IDs, không command strings từ UI/MCP/log. Mỗi host master/socket riêng; connect10s, keepalive15s×3, reconnect1→30s có jitter và dừng sau5 phút/NeedsAction; user stop luôn hủy retry.

Tái sử dụng ControlMaster do app sở hữu cho nhiều source, giới hạn concurrent channel opens và xử lý MaxSessions/refusal; multiplexing giảm connection handshakes, không bảo đảm vượt mọi server limit. Warm source-open <50ms là target đo theo môi trường, không promise cold connect. Dùng SSH_AUTH_SOCK/IdentityAgent đã được user cấu hình cho SSH Agent/1Password; không tự cài/key-manager daemon. Dynamic config và platform-specific Keychain cần trust/platform support rõ, không hứa kế thừa 100% config tùy ý.

Remote operation policy là allowlist read/probe/follow templates, không có API arbitrary exec/write/delete/service-control. User gắn Production vào host profile; viền/badge/accessible label theo host xuyên views. Nhãn không cấp quyền, tự connect hay thay host-key policy. Read-only áp dụng thao tác TailPro trên VPS; server vẫn có thể ghi audit logs do kết nối SSH.

## Discovery 3 tầng

### Chính sách và kết quả chung

Discover Sources mặc định chỉ đọc metadata trong scope user chọn; không tail/sniff nội dung log. Đọc config là capability riêng được user bật với danh sách file/roots rõ; chỉ extract path hints, không lưu/echo config/env/secret. Bấm Start sau khi chọn source mới cấp phép đọc log. MCP chỉ list/read nguồn đã grant, không tự discovery rộng hay mở nguồn mới.

Mỗi probe≤5s/1MiB output, concurrency≤2/host; discovery session≤15s/1MiB aggregate/500 candidates,≤1000 directory entries. Timeout/cap trả partial+reason; cap trước enumeration/allocation, không glob toàn bộ rồi head. Cache metadata≤30s có byte cap trong global control pool, invalidate khi generation/scope đổi. Cancel đóng owned tasks/processes; remote timeout/cleanup ngăn probe mồ côi.

Candidate fields: opaque ID, host/path, provider/provenance, observed_at, type, size/mtime nếu có, process hint, access và confidence. Dedupe metadata bằng path/identity đã xác minh, không đọc file để nhận dạng. Tag là gợi ý, không bằng chứng runtime/framework.

| Nhãn UI | Ý nghĩa |
| --- | --- |
| Ready | Regular-file metadata và readable_at_probe; Start phải kiểm tra lại trên descriptor |
| Permission denied | Có evidence EACCES/quyền thiếu; không suy mọi test-r=false là denied |
| Missing/Unknown/Unsupported | Không tồn tại, chưa đủ evidence, hoặc reader/tool không hỗ trợ |
| Opened / Recently modified | FD đã thấy hoặc mtime gần; không khẳng định đang ghi log |
| Active (observed growth) | Metadata size tăng giữa hai lần quan sát có timestamp; chỉ là activity hint, không chứng minh writer/process |

### Tầng 1 — Manh mối process/config

- Docker: bounded CLI listing với metadata Compose chọn lọc, không inspect toàn environment.
- Systemd: bounded list tên/trạng thái running services trong discovery scope/caps; selected units mới probe properties bên dưới. Không gọi start/stop hoặc đọc journal content khi Discover. Journal source cần adapter P4 và user Start.
- Process: current-user/explicit selected PIDs/services; cap32 PIDs×64 FDs, skip khi /proc bị giới hạn. Đọc readlink/fdinfo metadata, lọc regular candidates `.log/.out/.err` hoặc explicit config path; loại pipe/socket/anon/deleted handles khỏi Ready. Binary tự compile không nhất thiết có tên go; không dùng allowlist tên ngôn ngữ như bằng chứng đầy đủ.
- Không mặc định `lsof +D`: option này duyệt toàn cây và có thể tốn RAM/thời gian. Nếu lsof được chọn làm fallback thì chỉ PID/FD đã scope, cap tương tự. FD mở không chứng minh process đang ghi hoặc file là log. [lsof](https://lsof.readthedocs.io/en/latest/manpage/), [proc fd](https://man7.org/linux/man-pages/man5/proc_pid_fd.5.html).
- Config opt-in: bounded providers đọc tối đa64KiB/file,256KiB tổng/session trong cap1MiB; includes tối đa depth4/32 files, cycle detection, resolved targets trong approved roots. Không execute template/variables hoặc service commands để suy đường dẫn. Config trên disk có thể khác config process đã load: gắn configured_hint, không gọi chắc chắn active.

| Provider | Chỉ lấy các manh mối này |
| --- | --- |
| Nginx | access_log/error_log từ approved /etc/nginx files/includes; off/syslog/stderr không phải file; relative path thiếu runtime prefix và variables để unresolved |
| Caddy | output file trong Caddyfile hoặc filename của JSON file writer; không dùng parser access_log của Nginx; dynamic import/placeholders để unresolved |
| PM2 | List metadata remote ~/.pm2/logs hoặc PM2_HOME user chỉ định; không gọi pm2 CLI/jlist/daemon, không đọc pm2_env |
| Supervisor | stdout_logfile/stderr_logfile trong approved config; AUTO/NONE/interpolation chưa resolve không đoán thành file |
| Systemd | Selected unit properties StandardOutput/StandardError/FragmentPath/DropInPaths; chỉ file:/append:/truncate: là file hints. journal gợi adapter P4; không lấy Environment/ExecStart toàn bộ |

Parser theo từng format, không grep chung rồi coi path là đúng. Không quét /etc hoặc /proc vô hạn. Nguồn: [Nginx](https://nginx.org/en/docs/http/ngx_http_log_module.html), [Caddy](https://caddyserver.com/docs/caddyfile/directives/log), [Supervisor](https://supervisord.org/configuration.html), [systemd](https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html).

### Tầng 2 — Shallow scan có scope

Chỉ /var/log depth≤2 và project root do user nhập/chọn. Với project, probe các child logs/, log/, storage/logs/, tmp/logs/ theo cùng entry/time/byte caps; không walk toàn /opt, /home, /var/www. Symlink/cross-root expansion cần scope rõ, không follow mặc định. Extension/mtime chỉ là ranking hints: xét regular files cả khi không có đuôi .log, ưu tiên .log/.out/.err và mtime7 ngày; retain explicit/config candidates cũ hơn nếu có lý do. Sort bounded metadata ở client, không chạy ls -lt trên cây lớn.

test-r chỉ là probe; phân biệt missing/denied/unknown bằng metadata phù hợp, lỗi một file không hủy danh sách. Mtime mới gắn Recently modified, không gọi chắc chắn Active; FIFO/device không được mở thử để xem nội dung. Start dùng type/identity/permission kiểm tra lại để xử lý TOCTOU.

Active chỉ dùng khi metadata size đã tăng trong hai bounded observations; ghi observed_at/window, reset khi identity đổi và không biến UI refresh thành polling mọi candidate. Chỉ quan sát tối đa20 visible/selected candidates, cách nhau≥1s trong session caps; thiếu evidence giữ Recently modified/Opened/Unknown.

### Tầng 3 — Command palette

Autocomplete debounce200ms, latest-wins,≤1 request đang chạy/host, deadline1s,≤20 results/200 entries/64KiB trong shared discovery caps. Input là typed parent directory + literal basename prefix; không ghép wildcard/pipe vào ls shell string. Names có spaces/quotes/control phải có safe framing/escaping; unsupported paths báo rõ.

Tag nginx/postgres/app/worker theo evidence/path, cho user sửa. Lưu tối đa100 successful/pinned paths/host trong settings tổng≤1MiB, bound host profile+remote identity; pin không tự grant hoặc auto-stream. Mở lại phải recheck access/identity. Không lưu raw log hoặc config content trong history.

## Ingestion và event

Physical record position → bounded sanitizer/framing → parse/level → per-source multiline → normalized event → admission/disposition ACK. JSON strings sanitize cả sau unescape. Raw giữ bounded để Inspector escape khi render; không replay ANSI/HTML/URL commands.

Frame≤256KiB, depth≤64; selective visitor, không eager Value/tree mỗi record. Canonical buffers/extracted metadata có work/allocation cap; malformed/oversize fallback text/flags rõ. Inspector lazy≤4096 nodes,≤200 children/expand trong UI budget. Numeric time/duration phải có unit/profile; giữ event_time và local observed_at cùng quality, không đoán causal order giữa servers.

IDs là opaque strings; u64 khi qua JSON dùng decimal string, không mất precision trong JavaScript. Wire DTOs versioned và generate từ Rust; compact store representation tách khỏi DTO. Source candidate giữ provenance/access riêng; ReadResponse chứa schema_version, authorized scope/events, coverage/time_basis/complete/gaps/truncated và optional next_cursor. Không copy nguyên desktop DTO có global ingestSeq sang MCP.

Basic Go panic/Python traceback/Node error từ phase 1 (mục 1.3): chỉ nối continuation vào recognized error group cùng source/stream/generation; không gom tùy ý mọi indent. Group≤200 lines/256KiB, idle50ms/max age100ms; late context≤2s tạo immutable part cùng groupId/inherited level/quality. New JSON/header, rotation/stop/disconnect/cap phải flush đúng; custom regex profiles để backlog sau 5 phase. level:error giữ stack parts còn retained.

## Checkpoint và resume

Checkpoint RAM xuyên SSH reconnect, không persist qua app restart. Source physical sequence/ranges và generation cấp trước assembly. Engine ACK chỉ advance qua contiguous physical prefix đã accepted hoặc có committed gap; group sau không vượt group trước pending. Ledger≤256 outstanding/source; partial record chưa disposition không advance. Received, accepted, discarded, evicted và delivered là các counters khác nhau.

| Nguồn | Contract |
| --- | --- |
| Docker | Container ID + committed last_seen_timestamp RFC3339Nano; reconnect dùng --timestamps --follow --since timestamp, không thêm --tail200 làm cắt downtime. Boundary policy được kiểm chứng, có thể lùi1ns nếu cần. Ledger4096 keys/512KiB chỉ phân loại replay; giữ suspected repeats, không hash/occurrence hard-suppress legitimate records. Clock/order/retention ambiguity có gap/uncertain; restart cùng ID giữ cursor, recreate ID mới không reuse |
| File | Device/inode/generation + zero-based next-byte offset trước decode/ANSI/grouping; cùng identity/size/guards resume offset, GNU tail tương đương -c +offset+1. Rotate: Log rotated marker rồi file mới từ0; truncate/guard mismatch: Log truncated, reset0/uncertain. Guards không chứng minh exactly-once khi copytruncate/regrow/inode reuse |
| Journal P4 | Opaque cursor sau committed record; vacuum/quyền đổi có bounded fallback và gap, không hứa durable history |

File reader: template chỉ đọc qua SSH, open descriptor trước rồi metadata/ranges từ descriptor; không stat(path)+tail(path) làm proof. Chỉ seekable regular file, bounded1s open/type handshake để tránh FIFO race, kiểm tra /proc reopen permissions. Chunk≤64KiB, byte seek không O(offset), initial recent window resync newline và ghi absolute start. Idle poll≥250ms,≤1 helper/source, không spawn mỗi dòng; P1 phải chứng minh remote process/CPU overhead. Metadata/payload framing tách biệt và charge encoded+decoded buffers. Không lấy lại được bytes đã xóa; fail hoặc fallback có continuity unknown, không đánh dấu file-resume gate đạt khi chỉ có tail trần.

## Query và MCP cho coding agent

Live order theo local ingest sequence. Alpha query: text literal, AND, level/source/server/message/trace_id/order_id/user_id/duration comparisons với ms/s; tối đa4KiB/32clauses, deadline2s, UI debounce150ms. Unknown/unsupported syntax trả lỗi; missing field không match. Không thêm OR/NOT/regex vào alpha.

MCP P3 có bốn baseline tools: list_sources, get_recent_logs, search_logs, get_log_context. phase 4 thêm bốn tools trong mục Agent tools chuyên sâu; giữ baseline để observation/recent paging tương thích, tổng tám tools và advertise version/capabilities. Default response≤16KiB/50events, hard≤500events/1MiB, deadline2s,≤2 concurrent/client và10requests/phút/client cho toàn bộ tool set. Authenticated external search/predicate/count/context chỉ trên authorized redacted projection; cấm raw-search rồi mới mask output. Recheck grant trước send, không lộ global IDs/sequence hoặc scope ngoài grant.

Agent dùng model/code/commands của client; viewer không read repo, edit, exec hoặc deploy. Context có evidence refs, time basis, source health, coverage/gaps/truncation. Code hints từ sanitized stack chỉ relative path trong user mapping, reject traversal/absolute/URL/command;≤32 hints, path≤1KiB; revision/image/git claims có provenance, không chứng minh deployed code.

get_recent_logs(limit=0) capture metadata-only observation baseline ngay trước reproduce sau external build. Cursor khác page cursor30s và source checkpoint; opaque handle bound principal/session/grant/profile/source generations/query chain.≤2handles/client,16global,TTL10 phút, metadata≤64KiB; không pin raw retention. Event refs LRU≤4096/1MiB/TTL10 phút trong bridge budget, expiry yêu cầu search lại.

Incremental read scans after cursor tới captured fence; next cursor chỉ advance last-scanned position, không skip matches chưa trả khi cap. Continuation single-flight; cancel không advance; changed query/source/grant invalidate theo chain. Retention/gap/timeout trả incomplete; TTL không bảo đảm logs còn. Không thấy error hoặc không thấy expected request phải báo chưa đủ evidence, không kết luận fix/pass/deployed. Log là untrusted data, không instruction của agent.

## Agent tools chuyên sâu

| Tool phase 4 | Kết quả và giới hạn ý nghĩa |
| --- | --- |
| locate_service | Literal match service/project/host aliases trong granted source metadata; trả candidates/provenance/ambiguity, không tự Discover/Start |
| get_error_clusters | Deterministic signatures trên authorized redacted errors; count/window/sample refs/coverage, không coi sample count là production error rate |
| trace_id | Typed trace_id hoặc order_id exact-match trong scope/time; trả bounded cross-source timeline và evidence, không chứng minh causal spans |
| inspect_context | Resolve scoped evidence/group/trace refs, bounded neighbors/stack/health/gaps và optional safe relative code hints |

Reuse cùng grants/read policy/query permits/response limits/ref TTL; không scan raw rồi redact sau grouping. Clusters mặc định≤20 groups, hard≤128 buckets và tổng≤500 represented events/samples; bytes/deadline cap có thể dừng sớm với incomplete. Signatures có normalization version và collision handling, không log secret hashes. Code mapping chỉ do user khai báo; viewer không read/scan repo. Client tự kiểm tra code/revision và thao tác external build/reproduce.

## Correlation và topology

Waterfall chỉ có duration bars khi explicit duration hoặc matched span/start-end semantics có unit/time basis hợp lệ; event-only records hiện markers. Không trừ timestamp hai host để khẳng định latency nếu chưa có clock evidence. Cùng business ID chưa đủ chứng minh cùng request; hiển thị scope/window/ambiguity/missing span/coverage và inferred edges khác explicit spans.

Trace/graph job dùng query pool/CPU permits/deadline2s hiện có; result≤256 nodes/512 edges/256KiB, trace event pages theo cap500events/1MiB nhưng default16KiB vẫn áp dụng với MCP. Một current graph/view trong giới hạn views/versions có sẵn, không cache ngoài ledger; UI charge vào cache32MiB. Hit cap/deadline trả partial, không rút dữ liệu rồi báo complete.

Topology là capability opt-in riêng cho exact Compose files/approved roots và selected endpoint env names. Config parsing tĩnh trong discovery caps64KiB/file,256KiB tổng/session, include depth4/32files, không execute compose/interpolation/templates. Không đọc toàn environment, /proc/*/environ hoặc .env tự động. Nếu user cho phép endpoint env hints:≤16 tên,≤128bytes/name,≤1KiB/value,≤16KiB tổng trong cap config; remote probe project đúng tên trước truyền, provider không lọc an toàn được thì báo unsupported. Không chuyển nguyên env map về client rồi mới lọc.

Endpoint strings sanitize userinfo/query secrets trước giữ/graph; unresolved vars/imports có nhãn, không đoán secret. Service/dependency/endpoint edges ghi provenance configured/inferred, không gọi là actual traffic. Error overlay chỉ authorized retained observations cùng window; rate cần numerator/denominator hợp lệ và coverage rõ, thiếu denominator chỉ hiển thị count/unknown. Thay config/scope/grant invalidate result; không lộ tên/edges của source ngoài grant qua tool. Graph UI có thể xem metadata trong discovery capability đã cấp, quyền đọc log vẫn riêng.

## Redaction và prepared output

Một Redactor dùng cho export/MCP từ P3 và optional AI. Sau scope/projection và JSON-unescape/ANSI sanitize, scan nested/raw/text strings: sensitive keys, URL duplicate/encoded params token/secret/api_key/sig/access_token, URI userinfo, Bearer và compact JWT/JWE. Percent-decode tối đa2 lượt; candidate dài/hỏng/work cap phải mask rộng an toàn hoặc redaction_incomplete, không fallback raw. Counts/grouping cũng dùng sanitized authorized representation. Không log matched secret/hashes hoặc fetch URL trong log.

Export prepare tạo exact already-redacted serialized bytes≤8MiB/10k events,1global,TTL60s; AI optional≤64KiB/100events,1global,TTL60s. Charge query pool, bind digest/principal/session/selection/profile/grant; AI thêm exact provider/model/endpoint/consent. Eviction không đổi owned bytes; expiry/revoke/clear/profile/provider change phải reprepare/preview. Commit không reread raw; native destination có overwrite policy, private temp, bounded cancel/write, atomic publish và cleanup failure.

Incident export giữ schema/snapshot ID/source/window/time_basis/coverage/gaps metadata: NDJSON có typed metadata record, CSV có metadata columns để file độc lập không cần sidecar. CSV quote theo format; spreadsheet-safe mode mặc định neutralize cells bắt đầu bằng formula/control prefixes, ghi transform trong metadata và preview exact transformed bytes. Không tự gửi Jira/GitHub. Copy field trong Inspector là local explicit clipboard action, không tự chuyển log sang provider.

In-app AI mục 3.6 optional tắt mặc định: no automatic cloud fallback/redirect, cloudHTTPS, local literal-loopback/proxy bypass và local model thật; không suy localhost là local inference. Secret store Linux không có thì BYOK unavailable, không plaintext fallback. Model provider của external coding client có thể nhận log theo grant; UI nói rõ trước khi cấp quyền.

Explain Error đã có trong roadmap mục 3.6 theo yêu cầu, chỉ gọi sau preview/consent; không gửi qua server TailPro. Response phải phân biệt evidence refs với hypothesis, giữ untrusted output ở text view, không chạy command/model action. Provider có deadline30s, request≤64KiB/100events như prepared cap, response≤64KiB và tối đa1 request global trong query background budget; cancel/expiry/revoke phải dừng/đóng I/O, không queue vô hạn. Local inference và BYOK cloud là hai lựa chọn rõ ràng, không hứa cloud không rời máy.

## Local bridge

Stdio adapter nối runtime đang chạy qua private Unix socket; stdout chỉ protocol. App-only directory0700/socket0600, validate owner/no-follow; per-user cache fallback tương tự khi XDG runtime thiếu, path dài trả lỗi. Exclusive instance lock sống bằng fd, không unlink/recreate lock khi tranh; lock/listener/secret fd CLOEXEC, child inheritance allowlist.

Session socket path mới mỗi app restart, registry atomic sau listener ready, token bound client/session/grant. Connect100ms + handshake200ms, một registry-change retry trong startup deadline500ms; adapter không unlink/auto-SSH. App giữ lock mới GC verified orphan refused/unchanged inode/type/UID; timeout/backlog-full/EACCES không chứng minh stale. Graceful exit close/unlink đúng owned session; SIGKILL xử lý ở startup sau.

≤8 enrolled clients/8 accepted sockets/backlog8,≤2 preauth handshakes; request frame≤64KiB kiểm tra trước JSON allocation, response≤1MiB, buffers+decoded+refs+copies chung≤8MiB trong query64MiB. Idle authenticated socket60s; limits trước auth không cấp query consumers. UI reserved lane/memory theo architecture; no extra engine/store/inference service.

## Nguồn kỹ thuật quan trọng

- [OpenSSH config](https://man.openbsd.org/ssh_config), [Docker logs](https://docs.docker.com/reference/cli/docker/container/logs/), [GNU tail](https://www.gnu.org/software/coreutils/manual/html_node/tail-invocation.html).
- [Tokio blocking/cancellation](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html), [Unix socket lifecycle](https://man7.org/linux/man-pages/man7/unix.7.html), [flock inheritance](https://man7.org/linux/man-pages/man2/flock.2.html).
- MCP revision/SDK/client compatibility phải pin theo P1 evidence, không theo latest mù quáng; [official Rust SDK](https://github.com/modelcontextprotocol/rust-sdk). Các refs không thay thế số đo/runtime verification.

## Implementation alpha Phase 4

Wire schema 2 advertise tám tools, required arguments theo tool schema. Window của specialized tools dùng local `observed_at_ms`; typed correlation exact-match trên sanitized projection, không ghép timestamp hai host thành causal span. `trace_ref` trong cùng scoped LRU/TTL registry giữ selector/generations để `inspect_context` requery trace; `group_ref` mở representative/neighbor context, không hứa toàn bộ occurrence history. Baseline tools không nhận specialized window/correlation arguments để giữ observation cursor chain không đổi.

Canonical error signature v1 chỉ bỏ top-level timestamp/time/observed_at_ms/trace_id/order_id/request_id/duration_ms, giữ full representation còn lại và so equality khi digest trùng. Signature≤4 KiB/tổng≤256 KiB, cap groups/samples bên trên; missing masked error/stack evidence trả incomplete thay vì coi cùng message prefix là cùng stack. Counts chỉ retained observed samples. Relative code hints luôn từ developer mapping, revision unverified; không trả repo root cho client hoặc mở file local. Mapping change revoke grants/refs; source/policy revisions invalidate UI results.

Topology alpha dùng static JSON/block-YAML subset từ exact approved files, không tự resolve includes/merges/variables. Allowlisted endpoints hiện chỉ lấy literal config `environment` entries (≤16 names/16 KiB tổng), không có runtime env probe. Chỉ hostname matched configured service trở thành inferred edge; bỏ credential/query/value trước chuyển SSH. Một request tạo fresh snapshot; UI được consent sẽ poll descriptor metadata revision mỗi 15 giây trong existing view/query/probe lanes; change/unavailable invalidate snapshot và yêu cầu reread. Metadata không phải content-integrity proof; target/race validation vẫn mở. Giới hạn này không sửa contract mục tiêu; xem [acceptance](../process/phase-04-acceptance.md).
