# Phase 5 — Tối ưu, kiểm chứng và phát hành Linux

**Mục tiêu:** Biến bộ tính năng đã có thành bản Linux phát hành được, có số đo hiệu năng và bằng chứng an toàn.

**Điều kiện bắt đầu:** Các mục core/UI/MCP trước đó đã chạy; hoàn tất toàn bộ feature đã yêu cầu trước khi công bố full feature set.

Khi giao code, chỉ định một mục, ví dụ `5.1`. Mục khác trong phase là kế hoạch tiếp theo, không tự triển khai cùng. Trạng thái thực nằm ở [process](../process/current-status.md), thứ tự tổng ở [roadmap](roadmap.md).

## 5.1 — Đóng gói Linux

**Đầu ra:** Tạo AppImage/deb candidate chạy thật trên Linux mục tiêu để kiểm chứng trước release.

**Dependency:** 1.8, 2.5, 3.2, 3.5 và gate phase 4 · **Code:** `src-tauri/` bundling/platform và release tools.

1. Build trên Ubuntu 24.04 base với locked dependencies; package app/MCP binary/resources và ghi checksum/integrity.
2. Chạy packaged GUI + SSH agent + bridge trên runtime 24.04/26.04, dùng clean machine/VM thứ hai cho beta.

**Xong khi:** Artifact cài/mở/kết nối/đọc/MCP/thoát sạch trên các target đã công bố; ghi checksum và runtime evidence máy/VM thứ hai, không thay GUI run bằng headless check. Settings migration/rollback được kiểm chứng ở 5.5.

**Giới hạn:** macOS/Windows có hardware/signing/runtime gate riêng ở backlog sau 5 phase; không chặn Linux bằng thiếu Mac.

## 5.2 — Profile và tối ưu hiệu năng thực tế

**Dependency:** phase 1–4. **Code:** chỉ module có bottleneck đo được.

1. Đo 100k/500k fixture 512-byte và payload lớn; actual representation/allocations/copies/serialization/render cùng process-tree RSS.
2. Chạy sustained/burst, UI freeze, nhiều views, pinned retired bytes, hot15k/s + ba quiet2/s rồi quiet burst; kiểm tra source reservations và shared CPU/query budgets.
3. Sửa bottleneck nhỏ nhất, đo lại cùng environment; chỉ thêm index/SIMD/parallelism/cache khi có lợi chứng minh được.

**Xong khi:** có baseline/after p50/p95/max theo targets ở roadmap, đúng budgets, không drop im lặng hoặc phá fairness để đạt số. Đo cold/warm/edit/link builds theo compile guide; không cargo clean/release rebuild mỗi lượt.

## 5.3 — Review an toàn, recovery và resource lifecycle

**Dependency:** phase 3–4. **Code:** module đang vi phạm contract được xác định.

1. Manual checks remote quoting/config/include changes, metadata/config/env consent, unauthorized query/count/group và canary URL/URI/JWT redaction.
2. Thử file rotate/truncate/FIFO/giant seek, Docker boundary replay, contiguous ACK/generation races; snapshot→live/overflow/active-tail không mất event âm thầm.
3. Thử prepared-output expiry/revoke, stale socket/SIGKILL/CLOEXEC/slow reader, sleep/wake/network loss/shutdown; quan sát owned process/fd/lease release.
4. Với 3.6, thử local/cloud endpoint, redirect/proxy, secret-store unavailable, cancel/deadline/output cap và consent đổi sau preview.

**Xong khi:** không còn lỗi chặn correctness/security/resource caps; mọi cap/gap/incomplete có semantics đúng, limitation được công bố và không claim exactly-once. Review schema/capabilities trên cả hai coding clients.

## 5.4 — Workflow người dùng và onboarding

**Dependency:** phase 2–4; packaged candidate từ 5.1.

1. Cho ít nhất năm developer thử cài → chọn host/source → incident → external edit/build → baseline/reproduce → evidence; ghi thời gian/điểm vướng thực.
2. Đo warm multiplexed source-open và discovery time trên setup đã ghi; mục tiêu first value trong 60 giây là hypothesis cần kiểm chứng, không đồng nhất với cold network/auth mọi môi trường.
3. Chỉnh lỗi onboarding/focus/feedback trong scope hiện có; không thêm tính năng mới trong release gate.

**Xong khi:** có sample/feedback và kết quả workflow; chưa đủ sample ghi rõ release limitation, không giả product validation. Agent không thấy expected request phải báo chưa đủ evidence.

## 5.5 — Settings upgrade và release rollout

**Dependency:** 5.1–5.4.

**Code:** versioned settings adapter/migrations, packaging và release tools hiện có.

1. Kiểm chứng versioned settings migration/failure/rollback; pins/mappings/Production labels không mất, grants không nới scope khi upgrade.
2. Chạy lại critical packaged smoke khi artifact thay đổi, công bố version/build provenance/checksum/compatibility/known limitations và hướng dẫn rollback.
3. Chỉ công bố feature/platform đã có evidence; release đầy đủ chỉ sau tất cả mục tính năng yêu cầu hoàn thành, AI opt-in vẫn tắt mặc định.

**Xong khi:** package cài/nâng cấp/gỡ/khôi phục được trên các target đã công bố; không lộ secrets/raw logs trong artifact hoặc diagnostics, process phản ánh trạng thái thật.

## Gate kết thúc phase

AppImage/deb chạy thật Ubuntu 24.04/26.04 trên máy/VM thứ hai; target beta được đo, không silent gap/corruption/leak. Có feedback người dùng, migration/rollback và giới hạn công bố rõ. Chưa có target evidence thì chưa release target đó.

## Tài liệu chuẩn khi code

Đọc [architecture](../system-design/architecture.md), các mục liên quan trong [contracts](../system-design/contracts.md) và [compile guide](build-performance.md). Budget/behavior theo system-design; plan không thay đổi chúng. Acceptance dùng build/lint/typecheck/manual smoke/profiling; không viết automated tests.
