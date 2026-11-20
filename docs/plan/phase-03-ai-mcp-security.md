# Phase 3 — AI-native, MCP và output an toàn

**Mục tiêu:** Coding agent lấy được runtime evidence an toàn; export và Explain Error dùng chung redaction/prepared-output policy.

**Điều kiện bắt đầu:** Phase 1–2. Làm 3.1 trước các consumer; 3.3 → 3.4 → 3.5 theo thứ tự.

Khi giao code, chỉ định một mục, ví dụ `3.1`. Mục khác trong phase là kế hoạch tiếp theo, không tự triển khai cùng. Trạng thái thực nằm ở [process](../process/current-status.md), thứ tự tổng ở [roadmap](roadmap.md).

## Trạng thái implementation — 20/11/2026

Code 3.1–3.6 đã triển khai theo yêu cầu toàn phase; evidence và gate thực ở [acceptance phase 3](../process/phase-03-acceptance.md) / [current status](../process/current-status.md). Gate vẫn **PARTIAL**: Codex đọc/context và hai CLI handshake đã chạy; Claude domain workflow, Ollama/BYOK thật, native integration/resource/race matrix và prerequisite phase 1–2 còn thiếu. Alpha dùng caps thấp hơn ceiling: export 2 MiB, bridge response 256 KiB, refs 1536; không thay contract budgets. Không tự đánh dấu các tiêu chí “Xong khi” bên dưới là accepted đầy đủ.

## 3.1 — Redaction dùng chung cho output

**Đầu ra:** Output sang file hoặc agent dùng một sanitized projection nhất quán.

**Dependency:** 1.3, 1.4 · **Code:** `app-core/application/` Redactor/read policy, synthetic secret canaries.

1. Sanitize nested keys/text, URL query duplicate/encoded params, URI userinfo, Bearer và JWT/JWE sau JSON-unescape.
2. Bound decode/work/allocation; input không xử lý an toàn phải mask rộng hoặc redaction_incomplete.
3. Expose authorized redacted representation cho predicate/count/group/context; giữ secret khỏi diagnostics và không fetch URL.

**Xong khi:** Kiểm tra thủ công canary JSON, token URL, postgres://user:pass@, Bearer/JWT, malformed encoding và huge candidate. Hai log chỉ khác secret cho cùng kết quả search/count sau projection.

**Giới hạn:** Không hứa nhận diện mọi bí mật tự do; mọi consumer dùng chung policy và báo incomplete, không fallback raw.

## 3.2 — Incident snapshot NDJSON và CSV

**Đầu ra:** Xuất cụm sự cố đã che secret cùng metadata để đính kèm issue.

**Dependency:** 2.2, 2.3, 3.1 · **Code:** `src/features/export/`, app-core prepared export, platform destination adapter.

1. Chọn scope/time/events; prepare exact redacted bytes kèm snapshot/source/time_basis/coverage/gaps metadata.
2. NDJSON có typed metadata record; CSV dùng metadata columns và quoting/cell neutralization theo export contract.
3. Preview payload trong caps, bind principal/selection/profile/grant/digest/TTL; commit đúng prepared bytes.
4. Native destination: overwrite rõ ràng, private temp + atomic publish, cleanup khi cancel/failure.

**Xong khi:** Thử eviction sau preview, grant revoke/expiry, CSV multiline/formula-looking values, disk failure và cancel. File khớp preview, còn metadata để biết incomplete, không chứa canary secret.

**Giới hạn:** Đây là chia sẻ file thủ công, không tích hợp gửi Jira/GitHub hoặc cloud storage tự động.

## 3.3 — MCP adapter, quyền và local bridge

**Đầu ra:** Kết nối coding client với application đang chạy qua MCP stdio có grant rõ ràng.

**Dependency:** 1.1, 1.7, 3.1 · **Code:** `crates/{app-protocol,mcp-adapter}/`, app-core local bridge/policy, settings grants UI.

1. Feasibility pin SDK/protocol và hai client/version thật; extract wire DTO khi phase 3 triển khai, giữ SDK ngoài core.
2. Dựng stdio adapter→private Unix socket→application API; token/session/grant và reserved query lane.
3. Thêm runtime path/lock/registry lifecycle, startup deadlines và orphan cleanup đúng owner; stdout chỉ protocol.
4. UI onboarding/consent nói rõ client/model bên ngoài có thể nhận log; revoke có hiệu lực trước send.

**Xong khi:** Handshake thực với hai clients; kill app bằng SIGKILL rồi reconnect không treo/stale unlink sai. Thử slow/preauth clients, fd inheritance, revoke, socket permissions và app unavailable.

**Giới hạn:** Adapter không tạo engine/SSH khác hoặc tự exec code; chưa thêm domain tool chuyên sâu trong feature này.

## 3.4 — Bốn MCP tool đọc log nền tảng

**Đầu ra:** Agent đọc đúng nguồn đã cấp quyền, nhận evidence và coverage thay vì dump toàn buffer.

**Dependency:** 2.2, 3.1, 3.3 · **Code:** `app-core/application/agent_context.rs`, app-protocol schemas, mcp-adapter tool bindings.

1. Implement list_sources/get_recent_logs/search_logs/get_log_context trên application API dùng chung engine.
2. Authorize scope trước projection/query; redact trước predicates/counts/context và recheck grant trước send.
3. Bound response/rate/concurrency, opaque evidence refs và expired/incomplete errors; cấm lộ global sequence.

**Xong khi:** Hai coding clients truy vấn cùng incident và mở context từ ref; unauthorized source không lộ qua counts/errors. Thử secret-search equivalence, response cap, rate limit và revoke race.

**Giới hạn:** Đây là baseline alpha bốn tools; phase 4 thêm locate_service/get_error_clusters/trace_id/inspect_context ở các mục đánh số tương ứng.

## 3.5 — Observation cursor cho vòng sửa code

**Đầu ra:** Agent lấy baseline trước reproduce và đọc phần log mới sau đó mà không bỏ qua matches khi paging.

**Dependency:** 3.4 · **Code:** `app-core/application/agent_context.rs`, cursor registry, existing MCP read schemas.

1. Capture metadata-only baseline bằng get_recent_logs(limit=0); bind scope/query/grant/source generation với opaque TTL handle.
2. Scan tới fence, next_cursor=last-scanned; single-flight continuation, cancel không advance.
3. Trả coverage/retention gaps/expiry và evidence refs; phân biệt observation cursor/page cursor/source checkpoint.
4. Onboarding workflow: external edit/build → baseline → reproduce → logs/evidence; viewer không chạy các bước ngoài.

**Xong khi:** Cap response giữa nhiều matching rows rồi lấy trang tiếp không skip; query/grant đổi làm cursor invalid. Không có expected request hoặc có gap phải trả chưa đủ evidence, không báo verified fix.

**Giới hạn:** TTL không pin retention và không chứng minh code deployed hay fix thành công.

## 3.6 — Explain Error qua Ollama hoặc BYOK

**Đầu ra:** Người dùng chủ động chọn lỗi để nhận giải thích có log evidence trong app.

**Dependency:** 2.3, 3.1, 3.2 (prepared-output pattern) · **Code:** `app-core/application/` optional explainer, provider adapters, Inspector action/settings.

1. Thêm opt-in local Ollama hoặc BYOK; endpoint/model/credential policy theo contract, tắt mặc định.
2. Prepare/preview exact redacted selection, consent bound provider/model/endpoint/digest; cap/deadline/cancel và clear TTL.
3. Trả giải thích phân biệt evidence/hypothesis; model output là untrusted text, không execute actions.
4. Ghi rõ local inference hay BYOK cloud, không đi qua server của TailPro, không tự fallback cloud.

**Xong khi:** Thử local endpoint thực, proxy/redirect, unavailable model, missing OS secret store, revoke và endpoint đổi sau preview. Không gọi provider trước consent, không lộ canary secret.

**Giới hạn:** Được đưa vào lộ trình theo yêu cầu; làm sau core/agent workflow. Không cài model/SDK trong scaffold hay buộc MCP phụ thuộc tính năng này.

## Gate kết thúc phase

Hai coding clients/version đã pin dùng được baseline read tools và observation workflow; grant/revoke/redaction/cursor/socket crash có evidence. Không cần model để đạt agent-ready alpha nội bộ. Mục 3.6 có thể triển khai sau đường MCP nhưng phải hoàn thành trước khi công bố đủ bộ tính năng AI đã yêu cầu; opt-in/tắt mặc định, không tự fallback cloud.

## Tài liệu chuẩn khi code

Đọc [architecture](../system-design/architecture.md), các mục liên quan trong [contracts](../system-design/contracts.md) và [compile guide](build-performance.md). Budget/behavior theo system-design; plan không thay đổi chúng. Acceptance dùng build/lint/typecheck/manual smoke/profiling; không viết automated tests.
