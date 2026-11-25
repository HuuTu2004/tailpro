# Phase 6 — Evidence và giới hạn hiện tại

> Historical validation record. Synthetic examples, fixtures, raw observation artifacts and the `manual-fixtures` feature were removed during source cleanup on 25/11/2026. Commands mentioning them describe past runs and are no longer current setup instructions. Use `npm run contracts` for TypeScript generation and the desktop runtime for real sources.

Ngày: **25/11/2026, Asia/Bangkok**. Evidence gồm **snapshot foundations6.1** (lịch sử) và **implementation6.2–6.12 với log/CLI mô phỏng** ở mục cuối. [Plan](../plan/phase-06-universal-log-ingestion.md), [schema contract](../system-design/ingestion-schema.md).

## Environment và input

- Baseline Git `203deef`; workspace dirty trước khi bắt đầu ở năm file UI/preview. Những thay đổi này được giữ nguyên; evidence không phải của clean release artifact.
- Linux x86_64; Rust1.97.1, Cargo1.97.1; dev profile hiện hành, locked dependencies, incremental cache. Không thêm crate/dependency.
- Manually invoked `ingestion_schema` (historical artifact removed) dùng dữ liệu tổng hợp nhúng trong example; không network, SSH, daemon hoặc real log/secret.
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

## Gates tại thời điểm6.1 — lịch sử

- **6.1 contract/foundations: chốt và manual observed.** Schema capability matrix có phạm vi, nguyên tắc original bytes/terminators, field precedence/provenance, canonical ns strings và fragmentation ACK/disposition accounting.
- **Runtime migration: chưa chạy.** `Event.raw` vẫn lossy String, Docker/Journal vẫn có legacy timestamp overwrite, framer vẫn truncate. OriginalPayload view không làm retained data thành byte-exact. Những mục này thuộc6.2–6.3.
- **6.2–6.12: Planned.** Fragmentation/whole-record redaction, boundary-aware search, parser formats/multiline/custom rules/metrics/Kubernetes/durability chưa có runtime evidence.
- **Full Phase6 Gate A/B/C: chưa accepted.** Không chạy remote matrix, GUI native, packaged app, sustained performance hoặc RSS benchmark trong scope này.
- Không đổi desktop/MCP wire, không regenerate TypeScript; không viết automated tests. Next selected scope đề xuất6.2.

## Implementation6.2–6.12 — log tổng hợp/mô phỏng được user chấp thuận

Ngày25/11/2026, Asia/Bangkok. User chọn triển khai toàn bộ phase và xác nhận dùng log mô phỏng. Không truy cập production host, Docker daemon hoặc cluster thật. Git lúc bắt đầu `203deef` + dirty schema/UI; HEAD được cập nhật bên ngoài phiên sang `0c4b7d4` trong lúc làm. Không reset/revert các thay đổi UI, prototype/research khác; evidence là dirty workspace, không clean release certification.

### Scope và representation

| Mục | Code / local observation | Giới hạn |
| --- | --- | --- |
| 6.2 | `RawPayload` owns original bytes, optional invalid-UTF8 display overlay; default fragments64KiB, LF/CRLF/EOF/interrupted, checked offsets; mid-record file checkpoint; binary/repeated Journal parts + entry ACK | Journal JSON envelope cap128KiB: NeedsAction/unknown, giữ checkpoint trước entry; không claim giữ entry vô hạn. Text copy khác byte-exact export |
| 6.3–6.5 | Root alias priority/first duplicate independent traversal; exact decimal epoch and numeric namespaces; separate app/transport time; JSON/logfmt grammar/header/access profiles | Fixed configured timezone offsets, không IANA/DST; thiếu year/tz giữ unresolved. HTTP error evidence không đổi explicit logger severity |
| 6.6 | Six-family assembler,200 physical IDs/part, raw delimiter preservation, cap/late/revision/generation/header fences | Không tái tạo interleaving thiếu IDs; late/capped parts incomplete. Structural pretty JSON/custom multiline states chưa có profile v1 |
| 6.7 | Explicit local TOML preview/apply, bounded regex/known captures, raw-only rollback, session revision | Source-session config không persist/restart; v1 không nhận arbitrary scripts/custom namespaces/nested mapping DSL |
| 6.8 | Inspector hex capped16KiB, raw count/delimiter/provenance/fragment coverage; JSON/logfmt lazy fields; local source metrics; scoped sanitized MCP query counters/evidence; local gap sources | Local diagnostic counters không xuất sang MCP; external counters chỉ tính sau grant/window + sanitization. Full stage histograms/telemetry proposal chưa triển khai |
| 6.9 | File helper thật với file synthetic; Docker capability/Journal binary/cursor/error helpers qua CLI stubs | SSH/daemon/driver/cache/vacuum/quyền target thật Unrun |
| 6.10 | Baseline +100k/500k; level predicate bỏ per-row formatting; local fragment KMP; sửa floor-straddling admission bug | Chưa thêm Finder/SIMD/batch vì chưa có E2E evidence có lợi. Metadata tăng charge/giảm số rows retained |
| 6.11 | [Deferred RAM decision](../system-design/ingestion-durability-decision.md) | Không spool/WAL/durable ACK |
| 6.12 | Scoped CLI adapter, explicit context/namespace/Pod UID/container ID/restartCount/previous identity; sealed memfd validated config; plugin/tokenFile/impersonation/insecure TLS rejection; pre/post/poll checks | Linux+remote kubectl/static credentials; no init/ephemeral support; Pod-name API race/rotation/history uncertainty vẫn tồn tại. Gate B thật Unrun |

### Commands và môi trường

Ubuntu26.04 x86_64, i5-12400/12 logical CPUs, Rust1.97.1/Cargo1.97.1, existing dev profile/incremental cache; locked dependencies. Thêm direct `toml`1.2.1 đã có trong lock và bật serde_json arbitrary_precision để redactor không làm tròn decimal timestamp. Không thêm automated tests/framework.

```bash
cargo fmt --all -- --check
cargo clippy -p log-core -p app-core -p app-protocol -p tailpro-mcp -p tailpro-desktop --lib --bins --examples --features app-core/manual-fixtures --locked -- -D warnings
cargo build -p tailpro-desktop --locked
cargo run -p app-core --example export_contracts --locked
npm run build
cargo run -p log-core --example universal_ingestion --locked
cargo run -p app-core --example universal_observations --features manual-fixtures --locked
python3 fixtures/universal-ingestion/simulate_sources.py
cargo run -p log-core --example viewer_profile --locked -- 100000 422
cargo run -p log-core --example viewer_profile --locked -- 500000 422
```

**Final scoped fmt/Clippy/native build/contracts generation/frontend build: PASS. Core/application/CLI manual drivers: exit0; detailed observations bên dưới.**

Native checks dùng `PKG_CONFIG_PATH=/tmp/tailpro-phase6-build-deps/usr/lib/x86_64-linux-gnu/pkgconfig`: download/extract WebKitGTK/JavaScriptCore/libsoup và dev metadata phụ thuộc vào thư mục tạm, không cài/sửa system packages. Application example dùng disposable `XDG_RUNTIME_DIR`0700 và private bridge; credentials/canaries đều synthetic, không in enrollment tokens. Phần loopback SSH/native loaded window không được chạy trong đợt này.

### Observations

Checked-in `manifest` (historical artifact removed), `core driver` (historical artifact removed), `application driver` (historical artifact removed), `CLI simulation` (historical artifact removed). Input xác định, không seed ngẫu nhiên cho nội dung; IDs/private temp directories thay đổi theo run. Raw observations ở `core` (historical artifact removed), `application` (historical artifact removed), `sources` (historical artifact removed), `performance` (historical artifact removed).

| Evidence | Expected / observed |
| --- | --- |
| U-FMT/TIME/LVL | 26 manifest cases: root/nested order, alias/duplicate, Pino/Zap/large ns, numeric syslog/OTel, negative epoch/i128 minimum, quoted/malformed logfmt, Kafka/Redis/access/syslog and negatives. Defined expected fields agree with observed output; malformed extraction falls back to raw |
| U-RAW-01 | 38,414 bytes including invalid UTF8/NUL/CRLF/interrupted tail;302 units,301 fragments; reconstructed bytes exactly match input; one CROSS-HIT through boundary; offset overflow explicit |
| U-ML-01 | Go/Python/Node/Java/PHP/Rust each form one group in six samples;1001-line capped stack represents all1001 physical IDs, max200/part,802 incomplete linked parts; generation/revision/new timestamp header each produce two separate rows |
| U-QRY-01 | Local boundary search returns1; external searches for canary suffix/marker return0, incomplete=true, gap metadata retained; empty external query gets masked fragments; export_canary_visible=false. Query counters scoped/sanitized; whole-record decimal projection preserves1795194001123456789ns |
| Rules | Explicit preview spans `[time,0,20]`, `[level,21,25]`, `[msg,26,38]`; invalid version/lookaround rejected; live revision2 preserved after invalid import |
| ACK | One pending parent +300 terminal units: max ledger entries2; no cursor before parent; stale generation ACK ignored; parent ACK drains to0 |
| U-SRC/file | Real file helper preserves19 synthetic bytes; resume offset19 carries partial record start7/next_payload12; rotation changes inode/generation1; truncate resets offset0/generation2 |
| U-SRC/Journal | Stub binary MESSAGE bytes `61ff000d0a` preserved; repeated values emitted separately, only last completes entry; stale cursor produces gap + recent fallback; cap produces unavailable/unknown without ACK-able record |
| U-SRC/Docker | json-file/TTY/none/denied capability observations; no application diagnostics mixed into payload. Logging daemon compatibility remains unverified |
| U-K8S-01 | Probe/follow/previous, instance restart after~2s, denied, changed config and credential plugin rejection observed; owned stub process count0 after runs; no real daemon/cluster |
| UI | Browser preview1280×840:17 visible rows, no body overflow; Sources contains parser/Kubernetes/diagnostics; rule config visible, Apply disabled before preview. Inspector/E2E native workload remains unrun |

### Performance: cùng corpus khoảng512-byte JSON,25 scans/query

Baseline `203deef` extracted into an isolated temporary workspace with only log-core membership; its existing viewer_profile received the same optional payload-length argument422. Baseline lock membership rewritten offline in that isolated directory; repository lock retained. No concurrent builds during measurements. Each process ingests once then runs25 scans/query; p50/p95/max and scanned rows are in the artifact. `/usr/bin/time -v` measures the headless process, not desktop process-tree RSS.

| Workload | Baseline | Current |
| --- | --- | --- |
| 100k ingestion | 4,391ms;100k accepted/0 gaps | 3,351ms;100k accepted/0 gaps |
| 100k charged bytes | 187,536,166 | 234,350,452 |
| 100k level query p95 | 14.384ms | 11.798ms |
| 100k missing-message query p95 | 108.599ms | 104.862ms |
| 100k peak RSS | 151,692KiB | 182,084KiB |
| 500k ingestion | 21,987ms;499,966 accepted/34 gaps | 16,724ms;500k accepted/0 admission gaps |
| 500k retained rows | 196,793 | 157,476 |
| 500k charged bytes | 369,040,673 | 369,094,283 |
| 500k missing-message query p95 | 226.277ms over196,793 rows | 171.966ms over157,476 rows |
| 500k peak RSS | 302,576KiB | 296,444KiB |

Không diễn giải query500k nhanh hơn là thắng cùng retained coverage: schema mới giữ thêm evidence và mất nhiều rows hơn trong cùng byte cap. 100k scan cùng100k rows cho thấy level predicate cải thiện nhưng một số predicates khác chậm hơn; không claim tối ưu tuyệt đối. Managed source store vẫn trong352MiB phần được phép; metadata/dynamic payload đều charged. Allocations/record, copied bytes, sustained hot15k/s+3 quiet sources, remote CPU/network, migration peak old+new và UI/native E2E p95/RSS chưa được đo đầy đủ, nên performance gate vẫn Partial.

Admission bug được tái hiện hai lần ở input254252 với corpus cũ~350-byte: source nginx còn~16MiB, ba source khác110–128MiB. Own-source-only eviction không thể thu hồi straddling floor segment. Fallback mới ưu tiên own source; nếu own không có pinned excess và đang bị floor chặn thì reclaim largest unpinned excess borrower, không phá protected suffix. Pinned pressure vẫn từ chối source gây ra nó. Original500k corpus chạy lại accepted500k/discarded0; temporary probe đã xóa, không giữ debug logs/test harness.

### Gate disposition

**Code/build + synthetic/manual observations:** implemented trong capability v1 đã nêu. **Gate A runtime:** Partial (broad sustained/real-source/native matrix chưa chạy). **Gate B:** implementation/simulation, cluster thật Unrun. **Gate C:** Deferred. Không claim durable/exactly-once/zero-drop end-to-end hoặc support mọi format/version; RAM retention và upstream unknown coverage vẫn tường minh. Next action là runtime validation khi có target, không tự lắp daemon/cluster hoặc tăng caps để làm report đẹp hơn.
