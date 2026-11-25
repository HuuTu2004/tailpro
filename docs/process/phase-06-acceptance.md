# Phase 6 — Evidence và giới hạn hiện tại

Ngày: **25/11/2026, Asia/Bangkok**. Scope: **6.1 — schema/domain primitives**, không triển khai6.2–6.12. [Plan](../plan/phase-06-universal-log-ingestion.md), [schema contract](../system-design/ingestion-schema.md).

## Environment và input

- Baseline Git `203deef`; workspace dirty trước khi bắt đầu ở năm file UI/preview. Những thay đổi này được giữ nguyên; evidence không phải của clean release artifact.
- Linux x86_64; Rust1.97.1, Cargo1.97.1; dev profile hiện hành, locked dependencies, incremental cache. Không thêm crate/dependency.
- Manually invoked [`ingestion_schema`](../../crates/log-core/examples/ingestion_schema.rs) dùng dữ liệu tổng hợp nhúng trong example; không network, SSH, daemon hoặc real log/secret.
- Input gồm ba normalized ns clock literals, canonical/invalid/i128-boundary integer strings, bốn raw bytes `[97,255,13,0]`, hai contiguous ranges và bốn JSON legacy-parser cases. Không có random seed; đây là walkthrough xác định, không benchmark hoặc test harness.

## Build và manual observations

| Check | Kết quả |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy -p log-core --lib --example ingestion_schema --locked -- -D warnings` | PASS |
| `cargo check -p app-core -p tailpro-mcp --locked` | PASS; giữ re-export `engine::Disposition`, callers cũ compile |
| `cargo run -p log-core --example ingestion_schema --locked` | Exit0; các observations bên dưới |
| Documentation links/content và `git diff --check` | PASS; kiểm tra local targets, Markdown fences và whitespace, gồm cả file mới |

| Evidence | Expected | Observed |
| --- | --- | --- |
| 6.1-SCHEMA | Version nội bộ độc lập wire2 | `ingestion_schema_version=1`; không sửa DTO/protocol |
| 6.1-TIME | Chọn event → transport → observed, giữ ba clock độc lập | Ba run selection lần lượt `Event`, `Transport`, `Observed`; JSON timestamps đều strings |
| 6.1-NS | Giữ `1795194001123456789` và `9007199254740993` nguyên giá trị | Output decimal string giống input, không qua float |
| 6.1-BOUND | Canonical integer và signed128-bit bounds | `0`, `-1`, i128 MAX/MIN được nhận; `+1`, `01`, `-0`, `1.5`, hai overflow bị từ chối |
| 6.1-RAW | Borrow original bytes, checked range và caller cap | Bytes giữ `[97,255,13,0]`, `same_allocation=true`, `next_contiguous=true`; inverted range None, wrong length/over-cap false |
| 6.1-LEVEL | Namespace khác nhau, không auto map | JSON `pino`, `syslog`, `open_telemetry`, `unknown` riêng; origin `unknown` |
| 6.1-ACK | Giữ outcome và public API legacy | Example dùng `engine::Disposition::{Accepted,Gap}`; reverse dependents check PASS; không thay runtime ACK algorithm |
| 6.1-SIZE | Primitives không thêm fields vào retained Event | x86_64 `Event=320`, `EventTimes=48`, `OriginalPayload view=32` bytes; `Event` layout không đổi. Đây là sizeof, không RSS/heap/perf certification |

## Legacy parser findings chưa được sửa trong 6.1

| Case | Current observation | Trạng thái |
| --- | --- | --- |
| Root trước nested: `level=info`, `payload.level=error` | Info, malformed=false | So với case dưới cho thấy key-order dependency |
| Nested trước root, cùng values | Error, malformed=false | Không compliant schema mapping; sửa6.3/6.4 |
| Numeric `time=1795194001123`, `level=50` | Unknown, time=None, malformed=false | Numeric namespace/unit extraction chưa triển khai |
| Decimal `ts=1795194001.123456789`, `level=warn` | Warn, time=None, malformed=false | Decimal epoch parsing chưa triển khai |

Các case qua `Stream::{File,Stdout,Journal}` chỉ kiểm tra shared normalizer với stream tags; không chạy Docker/Journal envelope hoặc daemon và không chứng nhận clock separation ở reader. Contract walkthrough giữ rõ expected schema và observed legacy output; không đổi Unknown thành success giả.

## Gates còn lại

- **6.1 contract/foundations: chốt và manual observed.** Schema capability matrix có phạm vi, nguyên tắc original bytes/terminators, field precedence/provenance, canonical ns strings và fragmentation ACK/disposition accounting.
- **Runtime migration: chưa chạy.** `Event.raw` vẫn lossy String, Docker/Journal vẫn có legacy timestamp overwrite, framer vẫn truncate. OriginalPayload view không làm retained data thành byte-exact. Những mục này thuộc6.2–6.3.
- **6.2–6.12: Planned.** Fragmentation/whole-record redaction, boundary-aware search, parser formats/multiline/custom rules/metrics/Kubernetes/durability chưa có runtime evidence.
- **Full Phase6 Gate A/B/C: chưa accepted.** Không chạy remote matrix, GUI native, packaged app, sustained performance hoặc RSS benchmark trong scope này.
- Không đổi desktop/MCP wire, không regenerate TypeScript; không viết automated tests. Next selected scope đề xuất6.2.
