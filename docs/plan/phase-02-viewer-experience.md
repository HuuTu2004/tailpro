# Phase 2 — Trải nghiệm log viewer

**Mục tiêu:** Hoàn thiện bảng log hằng ngày: nhiều nguồn, query, Inspector, pause và điều hướng bàn phím.

**Điều kiện bắt đầu:** Phase 1; ưu tiên hoàn thành 2.1 trước các mục UI còn lại.

Khi giao code, chỉ định một mục, ví dụ `2.1`. Mục khác trong phase là kế hoạch tiếp theo, không tự triển khai cùng. Trạng thái thực nằm ở [process](../process/current-status.md), thứ tự tổng ở [roadmap](roadmap.md).

**Implementation:** Đã có code 2.1–2.5 theo yêu cầu toàn phase ngày 20/11/2026; validation/gate thực tế ở [acceptance phase 2](../process/phase-02-acceptance.md), chưa fully accepted.

## 2.1 — Bảng live log gộp nhiều nguồn

**Đầu ra:** Xem Nginx/API/worker từ nhiều host trên một bảng có cột cấu trúc và màu nguồn ổn định.

**Dependency:** 1.1, 1.4, 1.5, 1.6 · **Code:** `src/features/live-view/`, shared typed client/channels, app-core view jobs.

1. Hiển thị Time/Server/Source/Level/Message/trace_id/duration_ms từ DTO, stable source badges và bounded column selection.
2. Kết nối paged snapshot→live view, virtualization, coalesced notices/ACK và latest-wins result generation.
3. Giữ bounded page cache, release view/subscription khi đóng; expose source health/gap ngoài filtered rows và nhãn Production từ 1.8.
4. Chọn nguồn bằng các hàng theo host: All hosts, All của từng host và từng ô File/Docker/Journal. Combine sources gộp nhiều nguồn trên cùng bảng; All của host theo dõi thêm nguồn mới. Add path mở form nhỏ tại hàng đó, Start & view mới bắt đầu đọc; Discover sources mở công cụ discovery của đúng host. Columns là mục riêng. Giữ giới hạn tám nguồn hiện có, không tự dò hay mở file khi chỉ chọn bộ lọc.

**Xong khi:** Trace một ID qua ít nhất ba nguồn trên 2 host/4 sources; chỉ page cần thiết vào JS. Đo frame/serialization và freeze UI 30s không tăng queue vô hạn.

**Giới hạn:** Local ingest sequence quyết định thứ tự live; Inspector, pause và query nâng cao có mục riêng bên dưới.

## 2.2 — Tìm kiếm và Saved Views

**Đầu ra:** Gõ ID hoặc query đơn giản để lọc tức thì và lưu lại bộ lọc thường dùng.

**Dependency:** 1.3, 1.4, 2.1 · **Code:** `log-core/query/`, `app-core` view use cases/settings, `src/features/query/`.

1. Compile literal/AND/field filters và duration ms/s thành bounded QueryPlan; unsupported grammar có lỗi rõ.
2. Execute bounded scans với cancellation/deadline, paging và source scope; UI debounce/latest-wins.
3. Lưu Saved Views nhỏ trong versioned settings; restore query không tự cấp quyền hay start nguồn.

**Xong khi:** Thử exact order_id/trace_id, missing fields, duration units, malformed query và rapid typing. Đo p95 core/E2E theo roadmap; query cũ không đè kết quả query mới.

**Giới hạn:** OR/NOT/regex và index chỉ theo backlog sau 5 phase hoặc số đo bottleneck.

## 2.3 — Inspector JSON và raw text

**Đầu ra:** Click một event để xem JSON tree, raw text và copy field có chủ đích.

**Dependency:** 1.3, 1.4, 2.1 · **Code:** `src/features/inspector/`, log-core inspect facade, app-core bounded detail jobs.

1. Load detail theo event handle; parse/lazy-expand node/children trong budgets thay vì giữ tree toàn store.
2. Render raw escaped, stack group parts và quality flags; copy field/value là hành động UI rõ ràng.
3. Release detail pins khi đóng/đổi event; retained event đã hết hạn trả expired/requery.

**Xong khi:** Thử JSON nhiều key/deep, giant raw, stack parts bị eviction và chuyển selection nhanh. Inspector không vượt pin/page caps, không thực thi HTML/ANSI/URL từ log.

**Giới hạn:** Export hoặc gửi MCP/AI đi qua Redactor và policy riêng; copy UI cục bộ không tự upload.

## 2.4 — Pause và giữ vị trí đọc

**Đầu ra:** Scroll lên hoặc bấm Space để giữ vị trí, ingest nền tiếp tục và có badge log mới.

**Dependency:** 2.1 · **Code:** `src/features/live-view/`, app-core view/anchor state.

1. Tách follow-live và paused viewport; anchor bằng opaque event handle, không dựa index đang đổi.
2. Đếm arrivals từ bounded notices/watermark, không queue mọi dòng khi pause.
3. Thêm jump-to-live, resume và expired anchor handling khi retention vượt vị trí.

**Xong khi:** Pause khi burst rồi thao tác Inspector/filter; anchor không nhảy khi append. Khi eviction làm mất anchor phải báo rõ và chọn fallback, không pin vô hạn để hứa đứng yên tuyệt đối.

**Giới hạn:** Phím Space chỉ áp dụng khi focus phù hợp; shortcut routing chung ở 2.5.

## 2.5 — Command palette và phím tắt

**Đầu ra:** Chuyển host/source, tìm log và pause bằng bàn phím theo focus rõ ràng.

**Dependency:** 1.7, 2.2, 2.4 · **Code:** `src/app/` shortcut routing, `src/shared/ui/`, existing feature public actions.

1. Cmd/Ctrl+K mở palette chọn host/source; Cmd/Ctrl+F focus query, Escape đóng panel, Space pause khi viewport focus.
2. Reuse source autocomplete/action API; chọn candidate không tự cấp quyền, action Start vẫn rõ ràng.
3. Điều hướng keyboard/focus restore, accessible labels và shortcut help; dùng layout/platform modifiers.

**Xong khi:** Thử Linux keyboard-only, text input không bị Space/global shortcut chiếm, focus sau dialog/cancel đúng; palette không tạo discovery requests vượt caps.

**Giới hạn:** macOS modifier mapping có thể thiết kế trước nhưng chỉ công bố runtime support sau platform gate.

## Gate kết thúc phase

**UI refactor 20/11/2026:** Trang chính giữ bảng log, tìm kiếm và đúng mục Connections; Sources/Trace/Topology đóng/mở theo ngữ cảnh, Inspector không chồng với tool panel. Connections mở một cửa sổ native nhỏ, New connection có Test connection / Save / Cancel. Save không tự Connect; test không lưu profile hoặc thay đổi reader đang chạy. Bảng log và Event details dùng một khung chung: bảng mặc định full width, chọn dòng rồi bấm Details/Enter mới mở chi tiết, kéo divider để chia lại độ rộng, đóng chi tiết trả lại toàn bộ chiều rộng. Dòng dài có scrollbar ngang riêng và grid width theo nội dung. Nút Saved views được bỏ khỏi toolbar. Export/explain vẫn dùng dialog riêng, Escape/focus và cache/anchor được giữ. Đây là thay đổi UI cho 2.1–2.5 và quản lý profile của 1.2; không tự đóng các runtime gates bên dưới.

Một incident có thể theo ID qua ba nguồn, lọc lỗi vẫn giữ stack, pause không giật anchor còn retained. UI không giữ cả buffer trong JS, bàn phím/focus hoạt động và có số đo frame/query. Đây là viewer nội bộ; chưa gọi agent-ready khi phase 3 chưa đạt.

## Tài liệu chuẩn khi code

Đọc [architecture](../system-design/architecture.md), các mục liên quan trong [contracts](../system-design/contracts.md) và [compile guide](build-performance.md). Budget/behavior theo system-design; plan không thay đổi chúng. Acceptance dùng build/lint/typecheck/manual smoke/profiling; không viết automated tests.
