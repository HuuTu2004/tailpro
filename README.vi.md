[English](README.md) · **Tiếng Việt**

# TailPro

Ứng dụng local-first đọc log qua SSH, không cài agent lên VPS. Đã có Tauri/React shell, hai Rust core library, SSH multiplexing, Docker/file readers/discovery và viewer phase 2: bảng đa nguồn, query/Saved Views, JSON/raw Inspector, pause và phím tắt; phase 3 thêm redaction/export, MCP/grants/observation và optional Explain Error; phase 4 thêm Journal, clusters/service/code hints, trace và static topology. Tiến độ và các gate chưa đạt ở [process/current-status.md](docs/process/current-status.md).

## Chạy development

Cần Rust stable, Node/npm, OpenSSH và GUI Linux. Desktop cần GTK 3/WebKitGTK 4.1 cùng dependencies trong [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). Core libraries build được khi không có WebView. File/discovery yêu cầu Python 3 trên host; Docker source yêu cầu Docker CLI và quyền đọc container logs.

```bash
npm ci
npm run contracts
npm run tauri dev
```

Để build binary chứa frontend assets:

```bash
npm run build
cargo build -p tailpro-desktop --features custom-protocol --locked
```

Giao diện gọi native Tauri IPC trực tiếp. Chạy `npm run tauri dev`; `npm run dev` riêng chỉ phục vụ frontend, không có backend giả hoặc dữ liệu demo.

Ứng dụng chỉ dùng host key đã được tin cậy trong OpenSSH. Import alias không chạy `ssh -G` hay Match exec. Config dynamic cần consent riêng cho session; không có arbitrary command input. SSH agent/IdentityAgent hiện có được dùng trực tiếp, không tạo daemon key manager.

Connect host → mở cây HOSTS bên trái → bấm + cạnh host → nhập đường dẫn trong cửa sổ Add log path riêng → Add → double-click log hoặc Enter để bắt đầu đọc và mở tab. × trên tab dừng reader; chuyển qua tab khác vẫn giữ reader đang chạy. Combine sources cho chọn các log đang mở để xem chung. Đóng hết tab thì viewer ngừng query và hiện hướng dẫn mở log. File mới chỉ được pin sau khi mở thành công; thêm mục vào cây chưa cấp quyền đọc. Production là nhãn do user gắn, không thay quyền truy cập. Live viewer đọc từng page ≤200 rows/512 KiB; cache tối đa 2000 rows. Ctrl+F focus query; Space ở bảng pause/resume, Enter mở Inspector, Escape đóng panel. Bảng log và Event details chung một khung: chọn dòng rồi bấm Details hoặc Enter để mở; kéo divider chỉnh độ rộng, dùng scrollbar ngang ở đáy bảng cho dòng dài.

Bấm Connections hoặc + cạnh HOSTS mở thẳng form kết nối trong cửa sổ hệ điều hành riêng → nhập SSH alias/host → Connect. Không cần vào danh sách rồi Add hoặc Save trước. Test connection là tùy chọn, dùng phiên tạm và không lưu/thay đổi phiên đang chạy. Kết nối đã lưu có thể được chọn ngay trong form; chọn profile chưa tự Connect. Connect dùng API hiện có để lưu profile và bắt đầu kết nối; trạng thái SSH hiện trong cây HOSTS, không tự Start log. Tab log một dòng nằm sát trên cùng; Connections và Settings nằm ở cuối sidebar. Đã bỏ menu ⋯, tìm nhanh Ctrl+K và các panel Sources/Trace/Topology. Thêm absolute log path bằng + cạnh host, rồi double-click hoặc Enter để đọc; Ctrl+F focus bộ lọc log. Settings mở cửa sổ cài đặt riêng.

Settings lưu cục bộ, versioned/atomic, tối đa 1 MiB. Log retention và checkpoints ở RAM. SSH disconnect tự reconnect có giới hạn; Stop/Disconnect do user hủy retry. File rotation/truncate và continuity ambiguity có marker/quality. Replay Docker không hard-suppress nội dung trùng.

## Development checks

```bash
cargo fmt --all --check
cargo check -p log-core --lib
cargo check -p app-core --lib
cargo clippy -p log-core -p app-core --lib -- -D warnings
npm run typecheck
npm run build
```

Theo quy định repo, không viết automated tests. Kết quả kiểm chứng lịch sử ở [acceptance phase 1](docs/process/phase-01-acceptance.md) [acceptance phase 2](docs/process/phase-02-acceptance.md) và [acceptance phase 3](docs/process/phase-03-acceptance.md). Không dùng số đo local làm cam kết VPS/network hoặc compatibility Ubuntu 24.04.

## Tài liệu

| Cần biết | Tài liệu |
| --- | --- |
| Tiến độ, blockers, next action | [Process](docs/process/current-status.md) |
| Map code, ownership, budgets | [Architecture](docs/system-design/architecture.md) |
| Hành vi và bảo mật bắt buộc | [Contracts](docs/system-design/contracts.md) |
| Thứ tự, gate và performance targets | [Roadmap](docs/plan/roadmap.md) |
| Compile scoped và measurements | [Build performance](docs/plan/build-performance.md) |
| Quy tắc contributor/AI | [AGENTS.md](AGENTS.md) |

## Lộ trình

1. [Core, SSH và nguồn log](docs/plan/phase-01-core-ssh-sources.md)
2. [Trải nghiệm log viewer](docs/plan/phase-02-viewer-experience.md)
3. [AI-native, MCP và output an toàn](docs/plan/phase-03-ai-mcp-security.md)
4. [Microservice và production debugging](docs/plan/phase-04-production-debugging.md)
5. [Tối ưu và phát hành Linux](docs/plan/phase-05-hardening-linux-release.md)

## AI / MCP và incident output (Phase 3)

`cargo build -p tailpro-mcp` tạo `target/debug/tailpro-mcp`. Mở desktop, Start nguồn, vào **AI & MCP settings**, chọn đúng sources/client và xác nhận external client/model có thể nhận redacted logs. Dùng enrollment config được hiển thị một lần, thay command bằng absolute path đến binary; token không đưa vào issue/commit. Grant chỉ trong app session, TTL một giờ; restart/source configuration change yêu cầu cấp lại. Không sửa global client config tự động.

Tám tools/schema 2: `list_sources`, `get_recent_logs`, `search_logs`, `get_log_context`, `locate_service`, `get_error_clusters`, `trace_id`, `inspect_context`. Coding client tự edit/build; capture `get_recent_logs(limit=0, query, sources)` trước reproduce, rồi đọc `search_logs` với observation cursor và cùng query/scope. Tiếp tục `next_cursor` tới hết fence; evidence ref dùng để mở bounded preceding context. Cursor/page/ref có TTL, không pin retention. Gaps hoặc không thấy expected request là chưa đủ evidence của fix/deployment.

**Export incident** trong viewer dùng scope/query hiện tại; Inspector có export/Explain cho event đang chọn. Export preview là exact sanitized NDJSON hoặc spreadsheet-safe CSV, gồm metadata; nhập absolute destination và chỉ chọn overwrite khi muốn thay file. TTL 60 s, cap alpha 2 MiB; eviction không đổi prepared bytes. Cancel/revoke/config change có thể yêu cầu prepare lại.

Explain Error tắt mặc định. Apply local Ollama endpoint literal-loopback `/api/chat` với model local đã có, hoặc HTTPS OpenAI-compatible BYOK endpoint; key lưu qua OS Secret Service (`secret-tool`), thiếu store thì BYOK unavailable. Mỗi lần Explain phải prepare/preview rồi consent cho endpoint/model đó. Không proxy/redirect, không cài model, không tự fallback cloud, không qua server TailPro. Model text được hiển thị như hypotheses/untrusted text, không execute actions.

Implementation/gate cụ thể và commands manual ở [Phase 3 acceptance](docs/process/phase-03-acceptance.md); đây vẫn là internal alpha **PARTIAL**, chưa release certification.

## Production context (Phase 4)

Start Journal từ discovered systemd unit; cần Python 3/journalctl/quyền đọc và cursor chỉ giữ trong RAM. Settings mapping source/service → declared repo label/root không mở/index repo. Coding client tự kiểm tra relative stack hints và revision. Trace và Topology đã được gỡ khỏi desktop UI ngày25/11/2026; API/backend và evidence phase4 vẫn giữ. Backend topology cần consent exact Compose files/roots riêng, chỉ static JSON/block-YAML subset và allowlisted literal endpoint hostname hints, không đọc `.env`/runtime env hoặc execute Compose. Graph ghi configured/inferred provenance, retained error counts và unknown rate; kiểm tra approved config metadata và invalidation mỗi15 giây thuộc UI cũ đã gỡ. Xem [Phase 4 acceptance](docs/process/phase-04-acceptance.md) cho evidence và gate **PARTIAL**.
