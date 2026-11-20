# Kế hoạch phát triển — 5 phase

**Mỗi phase một file plan, các tính năng bên trong đánh số `1.1`, `1.2`, `2.1`…** Khi giao code, chọn đúng một mục để giữ scope; roadmap chỉ dùng quản lý thứ tự và gate.

| Phase | Plan | Phạm vi |
| --- | --- | --- |
| 1 | [Core, SSH và nguồn log](phase-01-core-ssh-sources.md) | 1.1–1.8: nền tảng, SSH/mux, parser/store, Docker/file, discovery, Production safety |
| 2 | [Trải nghiệm log viewer](phase-02-viewer-experience.md) | 2.1–2.5: bảng đa nguồn, search/Saved Views, Inspector, pause, shortcuts |
| 3 | [AI-native, MCP và output an toàn](phase-03-ai-mcp-security.md) | 3.1–3.6: redaction, snapshot export, MCP/grants, read tools, observation cursor, Ollama/BYOK |
| 4 | [Microservice và production debugging](phase-04-production-debugging.md) | 4.1–4.7: journal, error clusters, code hints, locate_service, trace/context, waterfall, topology |
| 5 | [Tối ưu và phát hành Linux](phase-05-hardening-linux-release.md) | 5.1–5.5: packaging, performance, security/recovery, dùng thử, upgrade/rollout |

## Cách thực thi

Ví dụ: “Code **1.7 — Discovery** trong [phase 1](phase-01-core-ssh-sources.md), chỉ làm mục này”. Đọc dependency trước; prerequisite còn thiếu ghi vào process và làm phần độc lập trong scope, không tự triển khai các mục khác. Chỉ làm trọn phase khi user giao rõ.

Mỗi mục: đọc design → code phần nhỏ chạy được → scoped build/lint/typecheck → manual acceptance/profiling phù hợp → cập nhật process. Không viết automated tests. Xem [compile guide](build-performance.md).

Thiết kế chuẩn ở [architecture](../system-design/architecture.md) và [contracts](../system-design/contracts.md); tiến độ thực chỉ ở [process](../process/current-status.md). Phase 1 đã có implementation; gate và giới hạn thực tế ghi ở process. Các mục nội bộ phục vụ quản lý, không đồng nghĩa số lượng chức năng thương mại độc lập.

## Gate tích hợp

- **Sau phase 1:** bounded core và SSH/Docker/file/discovery chạy thật; trust/cursor/fairness có evidence, estimate lại theo feasibility.
- **Sau phase 2:** viewer nội bộ điều tra được incident; bảng/search/Inspector/pause giữ đúng budgets, chưa gọi agent-ready.
- **Sau phase 3:** agent-ready alpha nội bộ khi hai clients dùng read tools/observation an toàn. Không cần model cho MCP; 3.6 opt-in có thể làm sau đường MCP nhưng cần xong trước khi công bố đủ bộ AI đã yêu cầu.
- **Sau phase 4:** đủ production context, tổng tám MCP tools; waterfall/topology phân biệt evidence với inferred/configured hints.
- **Sau phase 5:** phát hành AppImage/deb Linux với runtime/performance/security/upgrade evidence. Chỉ claim các feature/platform đã kiểm chứng.

## Mục tiêu đo

Đây là target chưa được chứng minh. Ghi environment/profile/seed/versions/cache, p50/p95/max, accepted/discarded/evicted và steady/peak process-tree RSS. 500k dùng fixture 512-byte; payload lớn chạm byte cap trước row cap.

| Metric | Alpha nội bộ | Beta/release |
| --- | --- | --- |
| Sustained ingest | 1k events/s, 30 phút | 5k/s, 60 phút |
| Query core / E2E p95 | ≤150ms/100k; ≤300ms | ≤200ms/500k; ≤400ms |
| Scroll frame p95 | ≤33ms | ≤20ms |
| Loaded process-tree RSS | Mục tiêu ≤800MiB | Mục tiêu ≤800MiB, không tính Ollama |
| Burst / UI freeze | 5k/s 10s; freeze 30s | 10k/s 30s; freeze 30s |
| Warm multiplexed source-open | Đo target <50ms trong điều kiện đã ghi | Không thay bằng cold-connect promise |
| First value | Đo hypothesis ≤60s trên workflow đã ghi | Không hứa mọi network/auth/setup |

Chạm cap phải trả partial/incomplete/error/gap phù hợp; drop im lặng không được tính là pass. Gate chi tiết nằm cuối mỗi phase.

## Backlog sau 5 phase

macOS/Windows/native Keychain, interactive encrypted-key UX, PM2 daemon/bus, Compose recreate tracking, OR/NOT/regex/custom multiline và indexes/disk spill. Chưa chốt phase mới; chỉ lập plan khi chọn nhu cầu cụ thể có hardware/budgets/estimate/rollback. PM2 file discovery đã thuộc 1.7; thiếu Mac/signing không chặn Linux.
