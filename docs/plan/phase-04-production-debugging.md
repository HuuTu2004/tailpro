# Phase 4 — Microservice và production debugging

**Mục tiêu:** Thêm journal, service locator/error clusters/request context và waterfall/topology có provenance.

**Điều kiện bắt đầu:** Phase 3; thứ tự riêng của từng mục ghi bên dưới, không tự mở cả phase trong một task.

Khi giao code, chỉ định một mục, ví dụ `4.1`. Mục khác trong phase là kế hoạch tiếp theo, không tự triển khai cùng. Trạng thái thực nằm ở [process](../process/current-status.md), thứ tự tổng ở [roadmap](roadmap.md).

**Snapshot triển khai 20/11/2026:** Đã thêm code 4.1–4.7 theo yêu cầu toàn file. Gate **PARTIAL**; xem [observations và giới hạn](../process/phase-04-acceptance.md). Journal thực/native/Claude domain matrix và target topology freshness/races chưa accepted. Các tiêu chí dưới đây vẫn là gate mục tiêu, không phải claim runtime đã đạt.

## 4.1 — Đọc Systemd Journal

**Đầu ra:** Chọn unit đã discover để đọc Journal có cursor và trạng thái quyền rõ ràng.

**Dependency:** 1.2, 1.3, 1.4, 1.7 · **Code:** `app-core/adapters/sources/` journal reader, source selection UI.

1. Read-only journalctl template với scoped unit và bounded framing; không start/stop/restart service.
2. Lưu opaque cursor sau contiguous ACK, giữ generation/cancel/checkpoint policy.
3. Vacuum/permission/cursor invalid có bounded fallback và gap, không giả file byte offset.

**Xong khi:** Thử unit có/không quyền, disconnect, vacuum và unsupported host. Khớp source/time/stream normalization và không claim lịch sử durable.

**Giới hạn:** Systemd metadata listing có từ 1.7; adapter này thêm log content khi user Start.

## 4.2 — Gom nhóm lỗi cho coding agent

**Đầu ra:** Tool get_error_clusters trả nhóm lỗi lặp lại và đại diện có evidence gọn.

**Dependency:** 3.1, 3.4 · **Code:** `app-core/application/agent_context.rs`, bounded grouping jobs, MCP schemas.

1. Group trong authorized redacted scope theo signature deterministic; bound buckets/work/samples, xử lý signature collision bằng representation rõ.
2. Trả count/window/source/representative refs và coverage/truncation; count là retained observed sample, không tỷ lệ toàn production.
3. Đo response bytes/calls/time trên cùng incident so với search thô; reuse query pool, không cache ngoài budget.

**Xong khi:** Secret-only differences không tách nhóm, unrelated stacks không bị gộp vì cùng level/message prefix. Nhóm incomplete/cap có nhãn; client mở representative bằng context tool.

**Giới hạn:** Không LLM inference hoặc kết luận root cause tự động; tool mới được advertise qua schema version phase 4.

## 4.3 — Ánh xạ service sang code local

**Đầu ra:** Cung cấp safe relative code hints cho coding client từ mapping developer chọn.

**Dependency:** 3.4 · **Code:** `app-core/application/agent_context.rs`, settings mapping UI/adapter.

1. Lưu source/service→repo label/root mapping do user khai báo; không đọc hoặc index repo.
2. Extract bounded sanitized stack hints, reject traversal/absolute/URL/command; metadata revision/image phải có provenance.
3. Expose relative path/line/evidence và mapping confidence trong context, để client tự đối chiếu checkout.

**Xong khi:** Thử stack hợp lệ, ../, absolute paths, malicious URL và mismatched revision. Hint không làm viewer đọc file hoặc khẳng định đúng deployed code.

**Giới hạn:** Mapping là gợi ý theo khai báo, không tự discover dependency graph.

## 4.4 — Tìm service cho AI agent

**Đầu ra:** Tool locate_service tìm đúng nguồn đã grant từ tên service/project/host.

**Dependency:** 1.7, 3.4 · **Code:** `app-core/application/agent_context.rs`, authorized source metadata view, MCP schemas.

1. Match bounded literal service/Compose/source aliases trên nguồn đã grant, trả candidate IDs và provenance/confidence.
2. Nhiều candidate phải trả ambiguity để agent chọn; missing/unavailable giải thích theo authorized scope.
3. Gắn hướng dẫn tool context/trace phù hợp, không tự mở source hoặc discovery mới.

**Xong khi:** Thử tên trùng trên hai host, alias nhập sai và service ngoài grant; kết quả không lộ metadata unauthorized và không tự Start nguồn.

**Giới hạn:** Không chạy lệnh quản trị, đọc env hay quét server để đáp ứng tool call.

## 4.5 — Truy vết request và context cho AI agent

**Đầu ra:** Tools trace_id và inspect_context trả hành trình request cùng evidence qua nhiều service.

**Dependency:** 2.2, 3.4, 3.5; hints dùng 4.3 nếu đã có · **Code:** `app-core/application/agent_context.rs`, bounded correlation jobs, MCP schemas.

1. trace_id nhận typed trace_id/order_id và scope/time; exact-match trên sanitized authorized projection, trả timeline grouped theo source.
2. inspect_context resolve scoped ref/group/trace, lấy bounded neighbors/stack/health/coverage và optional relative code hints.
3. Giữ baseline read tools để observation/recent paging tương thích; advertise capabilities/schema version, cùng rate/budget ledger.

**Xong khi:** Trace qua ba nguồn, ID bị trùng business scope, refs expiry và partial retention; không bỏ gap hoặc gọi log timestamp là causal span. Hai clients discover/call tool mới thành công.

**Giới hạn:** Correlation theo ID là evidence; không tự suy transaction toàn hệ thống khi scope hoặc clock không đủ.

## 4.6 — Waterfall theo trace_id hoặc order_id

**Đầu ra:** UI dựng timeline theo service từ trace context để soi đoạn chậm có bằng chứng.

**Dependency:** 2.1, 4.5 · **Code:** `src/features/trace/` (chỉ tạo khi làm feature), app-core bounded correlation DTOs.

1. Reuse 4.5 result, chuyển explicit span/start-end/duration thành bars theo service; click dẫn tới log evidence/Inspector.
2. Hiển thị clock/time basis, missing spans và inferred links; event-only logs hiện markers thay vì bịa duration.
3. Virtualize/cap nodes/events/edges theo contract; selection/filter/cancel reuse query lane.

**Xong khi:** Thử trace đầy đủ, chỉ event logs, clock skew, missing parent và partial retention. Bar duration chỉ xuất hiện khi có semantics hợp lệ; không kết luận bottleneck bằng khoảng timestamp không đáng tin.

**Giới hạn:** Không distributed tracing agent/instrumentation mới, không hứa causal waterfall từ mọi log text.

## 4.7 — Service topology có provenance

**Đầu ra:** Vẽ service graph từ Compose/config được phép đọc và gắn lỗi quan sát được.

**Dependency:** 1.7, 3.1, 4.2, 4.4 · **Code:** `app-core/adapters/discovery/` scoped topology provider, application graph job, `src/features/topology/`.

1. Xin capability riêng cho exact Compose files/roots và tên env được phép; parse tĩnh trong caps, không execute/interpolate hoặc đọc toàn env/.env.
2. Kết hợp service/dependency/allowlisted endpoint hints, redact credentials; edge configured/inferred có provenance, không khẳng định traffic thực.
3. Overlay authorized retained error counts; chỉ hiển thị error rate khi có denominator/window hợp lệ; node click lọc nguồn.
4. Bound graph/payload, invalidate khi config/grant đổi; export/MCP reuse policy nếu sau này có consumer.

**Xong khi:** Thử Compose includes/variables unresolved, endpoint chứa secret, services thiếu grant và edge chưa có runtime evidence. Không có secret/config raw trong graph; thiếu denominator không tô tỷ lệ giả.

**Giới hạn:** Discovery mặc định không đọc env; topology phải opt-in riêng và không cài agent trên server.

## Gate kết thúc phase

Tám MCP tools tổng cộng, hai clients discover/call tools chuyên sâu được; payload/call cost đo trên cùng incident. Waterfall không bịa duration/causality, topology cần capability riêng và không lộ env/secret. Journal có cursor/vacuum behavior đúng; thiếu host capability thì công bố giới hạn rõ, chưa gọi hỗ trợ đầy đủ.

## Tài liệu chuẩn khi code

Đọc [architecture](../system-design/architecture.md), các mục liên quan trong [contracts](../system-design/contracts.md) và [compile guide](build-performance.md). Budget/behavior theo system-design; plan không thay đổi chúng. Acceptance dùng build/lint/typecheck/manual smoke/profiling; không viết automated tests.
