# Nghiên cứu ingestion log đa nguồn: khả năng, giới hạn và bằng chứng

Ngày kiểm tra nguồn: **2026-11-20**. Phạm vi: tài liệu/spec/source upstream về Docker, Kubernetes, journal, syslog, file tail, multiline và buffering; đề xuất cách áp dụng vào TailPro. Đây là nghiên cứu thiết kế, không phải chứng nhận compatibility hoặc kết quả benchmark.

Nhãn **Upstream** chỉ hành vi được tài liệu hoặc mã nguồn được dẫn mô tả. Nhãn **Đề xuất** là suy luận thiết kế cho TailPro; chưa có nghĩa đã implement. Nhãn **Cần xác minh** chỉ điểm cần chạy với phiên bản, quyền và cấu hình thực tế. Các URL `latest`, `main`, `master` thay đổi theo thời gian; acceptance phải lưu phiên bản thật, không chỉ URL.

## 1. Kết luận ảnh hưởng trực tiếp đến thiết kế

**Đề xuất:** thay tuyên bố “3 transports / 4 formats / 100% log / zero drop” bằng capability matrix có phạm vi. File, Docker và journal là cách truy xuất nguồn; SSH là kênh thực thi/truyền dữ liệu; JSON, syslog và text là cấu trúc dữ liệu. Chúng không cùng một trục. Một JSON app có thể nằm trong Docker stdout, CRI file hoặc `MESSAGE` của journal. Hỗ trợ outer JSON chưa chứng minh hiểu schema app bên trong.

**Đề xuất:** mục tiêu hợp lý là *đọc được nguồn đã cấp quyền trong giới hạn đã công bố, giữ evidence của dữ liệu không parse được, báo mọi mất mát/biến đổi đã biết và sự không chắc chắn*. Không thể bảo đảm khôi phục dữ liệu upstream đã xóa hoặc chưa ghi. Docker có chế độ chủ động drop khi buffer đầy; journal có rate limit; logrotate thừa nhận cửa sổ mất dữ liệu của copytruncate. Các bằng chứng cụ thể nằm ở mục 3–6.

**Đề xuất:** tách bốn kết quả độc lập: tiếp cận được nguồn; nhận đủ bytes trong phạm vi truy xuất; diễn giải đúng cấu trúc; còn giữ được kết quả trong retention. “Có dòng trong UI” không chứng minh đủ lịch sử. “Không parse được JSON” không nhất thiết là mất dữ liệu. “Evicted khỏi RAM” khác “drop trước khi accepted”.

**Đối chiếu repo:** [contracts](contracts.md) đã có contiguous ACK, checkpoint RAM, fallback/quality, multiline bounds, continuity ambiguity; [architecture](architecture.md) đã bound tài nguyên và yêu cầu gap khi pressure. [current status](../process/current-status.md) còn ghi nhiều gate daemon/SSH/rotation chưa đạt. Nghiên cứu này củng cố các giới hạn đó; không chuyển các target thành guarantee.

## 2. Tách trục để mở rộng mà vẫn kiểm soát được

**Đề xuất:** capability thuộc về adapter + phiên bản + cấu hình, không thuộc nhãn format chung.

| Trục | Ví dụ | Cần khai báo |
| --- | --- | --- |
| Cách tiếp cận | SSH command, API có auth, local file | Quyền, identity, cancellation, timeout, host trust |
| Nguồn và lịch sử | Regular file, Docker logs, journal, Pod logs | Retention, replay, cursor, rotation, identity thay đổi |
| Envelope/framing | Newline, Docker multiplex, CRI P/F, journal JSON, syslog length prefix | Biên record, stream, partials, size cap |
| Payload | Text, JSON app, logfmt, access log, traceback | Schema/profile, timestamp units, precedence, lỗi parse |
| Chuẩn hóa | Level, message, event time, trace context | Provenance, confidence, missing/invalid/conflicting |
| Vòng đời | Accepted, rejected, retained, evicted, queried, exported | Đơn vị đếm và coverage riêng |

**Đề xuất:** đường xử lý nên phân biệt rõ:

```text
source identity + physical position
  → transport/framing
  → unwrap envelope + runtime partial reconstruction
  → application record recognition / bounded multiline
  → parse payload + normalize with provenance
  → accepted event or explicit disposition
  → advance contiguous source checkpoint
```

Runtime chia một dòng JSON thành nhiều phần khác với app phát stack trace nhiều dòng. Không chạy JSON parser độc lập trên từng CRI `P` rồi coi đó là malformed; không nối các JSON event hoàn chỉnh chỉ vì body có whitespace. Không nối qua source/stream/container attempt/generation. Sanitization phục vụ hiển thị và trích field phải giữ được mapping về physical ranges; không được làm sai offset trước decode.

**Đề xuất:** adapter mô tả `history_kind`, `resume_kind`, `stream_fidelity`, `retention_known`, `partial_support`, `permission_state`, `identity`, `version`. Ví dụ `resume_kind=timestamp_overlap` không quảng cáo tương đương `opaque_cursor_verified` hoặc byte offset trên cùng file identity.

## 3. Docker: nguồn stdout/stderr có điều kiện, không phải toàn bộ log container

### 3.1 Driver và retention

**Upstream:** Docker có blocking và non-blocking delivery; non-blocking không enqueue thêm message khi buffer đầy. `json-file` mặc định không rotate; `local` rotate mặc định. Đổi daemon logging config chỉ áp dụng container mới, container hiện có giữ cấu hình cũ. [Docker: Configure logging drivers](https://docs.docker.com/engine/logging/configure/).

**Upstream:** `docker logs` có thể không hữu ích khi remote logging driver tắt dual logging. App ghi file riêng thay stdout/stderr cũng không tự xuất hiện qua lệnh này. [Docker: View container logs](https://docs.docker.com/engine/logging/).

**Upstream:** dual logging tạo cache đọc local cho driver không hỗ trợ đọc. Cache rotate, có thể tắt; tài liệu nêu mất cache khi write driver thất bại và không retry cache entry, cùng rủi ro ring buffer drop khi ghi file chậm. Vì vậy local cache không phải archive đầy đủ hay bản sao bảo đảm độc lập với remote destination. [Docker: Dual logging, limitations](https://docs.docker.com/engine/logging/dual-logging/#limitations).

**Upstream:** raw `json-file` chứa `log`, `stream`, `time`; Docker cảnh báo các file này dành riêng daemon quản lý, external access có thể gây hành vi bất ngờ. [Docker: JSON File logging driver](https://docs.docker.com/engine/logging/drivers/json-file/).

**Đề xuất:** discovery ghi driver và TTY nếu đọc được bằng inspect được scope; không chỉ đọc daemon default. Start trả unavailable/denied/unsupported rõ. Không chuyển ngầm sang đọc `/var/lib/docker/...` khi CLI lỗi. Log app ghi file riêng cần explicit file source với quyền riêng. UI ghi “Docker stdout/stderr còn truy xuất được”, không “toàn bộ log container”.

### 3.2 Framing và stream fidelity

**Upstream:** CLI hiện inspect container; TTY dùng `io.Copy`, non-TTY dùng `stdcopy.StdCopy(stdout, stderr, response)`. Do đó TTY không đem lại hai stream riêng như non-TTY. [Docker CLI: `logs.go`](https://raw.githubusercontent.com/docker/cli/master/cli/command/container/logs.go).

**Upstream:** ví dụ source Moby v28 mô tả header 8 bytes, stream byte và payload length big-endian; decoder còn xử lý daemon system error. [Moby v28: `stdcopy.go`](https://raw.githubusercontent.com/moby/moby/v28.0.0/pkg/stdcopy/stdcopy.go).

**Đề xuất:** với CLI hiện tại, giữ stdout và stderr pipe riêng; stderr của command còn có thể chứa diagnostics, không tự coi mọi nội dung stderr là event level ERROR. Với TTY gắn `stream=combined/unknown`. Nếu sau này dùng Engine API, decode frame trước text/newline parsing, bound advertised length trước allocation, giữ lỗi control tách payload. Framing của API không phải JSON app, và kích thước frame không phải biên event app.

### 3.3 Reconnect không có offset duy nhất

**Upstream:** `--timestamps` thêm RFC3339Nano; `--since` là điều kiện thời gian, có thể kết hợp follow/tail. Thiếu timezone làm CLI dùng local timezone. [Docker CLI: container logs](https://docs.docker.com/reference/cli/docker/container/logs/).

**Đề xuất:** lưu container ID và committed envelope timestamp riêng app timestamp; reconnect dùng UTC có timezone và overlap đã kiểm chứng. Không dùng tail giới hạn nhỏ cho replay downtime. Timestamp không là event ID: hai record có thể cùng timestamp/nội dung, clocks hoặc ordering có thể gây ambiguity. Dedup hash cưỡng bức có thể xóa repeat hợp lệ; ledger bounded nên phân loại `possible_replay`, giữ record và báo chất lượng.

**Cần xác minh:** equality boundary `--since`, precision thật từng driver, stdout/stderr interleave, long line, partial cuối pipe, daemon restart, stopped container, same ID restart, recreate/new ID, remote driver cache disabled, driver/network/disk failure, downtime dài hơn retention. Chỉ một probe CLI stub không chứng minh matrix này.

## 4. Kubernetes: Pod log API và CRI file là hai capability khác nhau

**Upstream:** kubelet quản lý rotation; `kubectl logs` chỉ trả file log mới nhất. Mặc định giữ một terminated container cùng logs; Pod bị evict thì container và logs đi theo. `--previous` truy xuất incarnation trước, không là toàn bộ restart history. [Kubernetes: Logging Architecture](https://kubernetes.io/docs/concepts/cluster-administration/logging/).

**Upstream:** CLI có since-time/since, tail, limit-bytes, previous, prefix và max-log-requests. Tail mặc định thay đổi khi dùng selector; phải đặt policy rõ, không dựa default trong collector. [Kubernetes: kubectl logs](https://kubernetes.io/docs/reference/kubectl/generated/kubectl_logs/).

**Upstream:** đề xuất CRI mô tả timestamp/stream/tag/content; `P` là phần chưa hoàn tất, `F` là record hoàn tất hoặc phần cuối. Đây là design archive, nên đối chiếu implementation. [Kubernetes CRI logging design](https://github.com/kubernetes/design-proposals-archive/blob/main/node/kubelet-cri-logging.md).

**Upstream, cần đọc cả mã:** `cri-client/pkg/logs/logs.go` parse CRI stream/tag, bỏ newline của phần `P`. Comment đầu file nói không handle rotation, nhưng code hiện có watcher và nhánh reopen khi file được tạo lại. Không được lấy comment cũ làm guarantee rằng mọi follow luôn fail rotation, hoặc lấy nhánh reopen làm bằng chứng đọc được mọi rotated history. [Kubernetes cri-client: `logs.go`](https://raw.githubusercontent.com/kubernetes/cri-client/master/pkg/logs/logs.go).

**Đề xuất:** nếu bổ sung Kubernetes, có hai chế độ độc lập:

| Chế độ | Phù hợp | Giới hạn phải hiện |
| --- | --- | --- |
| Pod log API/kubectl | Troubleshooting từ quyền Kubernetes có sẵn | Latest-file history, thời gian replay, termination/eviction, API/quyền/node availability |
| Node file/collector có quyền | Cần physical CRI positions, nhiều file retention hơn nếu còn | Quyền node, symlink/rotation/GC, metadata lifecycle, CRI parser và partial assembly |

Identity cần cluster/context + namespace + Pod UID + container name + container ID/attempt, không chỉ Pod name. Selection nhiều Pod phải giữ provenance và giới hạn concurrency; prefix text không thay thế structured identity. Theo dõi Pod mới/restart là lifecycle/discovery riêng; một command follow thành công không chứng minh tự cover deployment/recreate.

**Đề xuất:** nối runtime partial theo stream/attempt trước schema app. Bound bytes/parts/age; phần mất trước `F`, timeout, restart và rotation phát `runtime_partial_incomplete`. Không ghép qua hai restart. API đã chuyển thành payload stream thì không giả vờ còn physical CRI offset/P-F đầy đủ.

**Cần xác minh:** Kubernetes version/feature gate stream selection, runtime version, rotation trong follow, multiple restarts khi offline, Pod delete/recreate cùng name, init/ephemeral containers, access-denied và expired credentials. Nghiên cứu không xác nhận TailPro hiện có adapter Kubernetes.

## 5. Journal: cursor mạnh hơn timestamp nhưng không giữ dữ liệu hộ client

### 5.1 Resume phải kiểm tra exact cursor

**Upstream:** `--after-cursor` đọc sau vị trí cursor; `journalctl` chỉ thấy journal mà user được phép đọc; vacuum xóa archived files theo các giới hạn. JSON output mặc định biểu diễn field lớn hơn 4096 bytes thành `null`, trừ khi bật `--all`. [systemd: journalctl](https://www.freedesktop.org/software/systemd/man/latest/journalctl.html).

**Upstream:** `sd_journal_seek_cursor` có thể seek vị trí gần nhất theo thời gian nếu không tìm đúng entry; sau seek/next cần `sd_journal_test_cursor` để xác minh match. Realtime timestamp cũng có thể ambiguous. [systemd: sd_journal_seek_cursor](https://www.freedesktop.org/software/systemd/man/latest/sd_journal_seek_cursor.html).

**Đề xuất:** “journalctl exit 0 và có output” không đủ để đánh dấu continuity verified. Helper dùng CLI cần bounded exact-cursor probe (so cursor returned bằng opaque string, không parse nội dung cursor), rồi mới resume; nếu không chứng minh được exact match thì đánh dấu `resume_unverified` và bounded recent fallback/gap. Không tuyên bố rằng mọi trường hợp `--after-cursor` khi vacuum đều sẽ exit lỗi: hành vi CLI/phiên bản phải đo riêng.

### 5.2 Envelope là dữ liệu có cấu trúc

**Upstream:** `__CURSOR` là opaque position; `__REALTIME_TIMESTAMP` là thời điểm journal nhận entry, microseconds; monotonic timestamp phải đi với `_BOOT_ID`. `PRIORITY` là decimal 0–7; field không có underscore không được journal validate như trusted fields. [systemd: journal fields](https://www.freedesktop.org/software/systemd/man/latest/systemd.journal-fields.html).

**Upstream:** journal cho phép binary data và field lặp. JSON representation dùng array cho repeated values/binary bytes; export format có framing nhị phân riêng. [systemd: Journal Export Formats](https://systemd.io/JOURNAL_EXPORT_FORMATS/).

**Đề xuất:** giữ `journal_received_time`, `_SOURCE_REALTIME_TIMESTAMP` nếu hợp lệ, app timestamp trong `MESSAGE`, và local observed time riêng. Không overwrite app event time bằng journal receive time. Decoder không ép `MESSAGE` luôn string; không thay repeated/binary message bằng placeholder rồi gọi raw complete. Có thể giữ bounded byte representation/structured values, hoặc công bố `binary_omitted`/`field_truncated` và byte count đã biết. `--all` gỡ upstream output omission nhưng không gỡ budget TailPro; cần cap ngay khi streaming JSON.

### 5.3 Journal vẫn có mất dữ liệu trước reader

**Upstream:** `Storage=volatile/persistent/auto/none` thay đổi nơi lưu; `none` không lưu received data. Rate limiting có thể drop thêm messages theo service; có notice số lượng bị drop, và unit có thể override limit. Size/retention settings giới hạn journal. [systemd source: journald.conf](https://raw.githubusercontent.com/systemd/systemd/main/man/journald.conf.xml).

**Đề xuất:** expose journal namespace, visible scope, retention/cursor verification state; không tự thay journald config. Suppression notice chỉ là bằng chứng loss upstream nếu đọc được và nhận diện đúng; absence của notice không chứng minh zero upstream loss. Cursor RAM qua app restart không tồn tại: UI cần phân biệt “bắt đầu cửa sổ mới” với “resume cursor đã commit”.

**Cần xác minh:** journalctl/version trên host, journal group/ACL, user-vs-system namespace, `MESSAGE` dài/binary/lặp, cursor bị vacuum, boot đổi, clock step, permissions bị revoke trong follow, exact-cursor probe đủ scope. Không cần/root không có nghĩa mọi journal đều đọc được.

## 6. Syslog và file tail: format không đem lại delivery guarantee

### 6.1 Syslog cần hai parser riêng: message và transport framing

**Upstream:** RFC5424 phân lớp message với transport. Header có PRI/version/time/host/app/procid/msgid/structured data; timestamp có thể là NILVALUE. PRI mã hóa facility và severity 0–7, số nhỏ nghiêm trọng hơn. MSG có thể là bytes, không mặc định tất cả UTF-8. [RFC5424, §6](https://www.rfc-editor.org/rfc/rfc5424.html#section-6).

**Upstream:** RFC6587 là Historic, mô tả octet counting và non-transparent delimiter framing. Delimiter LF trong body có thể khiến receiver tách nhầm; octet count tính bytes, không Unicode characters. RFC khuyến nghị TLS thay plain TCP. [RFC6587, §3.4](https://www.rfc-editor.org/rfc/rfc6587.html#section-3.4).

**Upstream:** UDP không có cơ chế sửa lost datagram hoặc bảo đảm thứ tự trong mapping syslog này. [RFC5426, §4](https://www.rfc-editor.org/rfc/rfc5426.html#section-4).

**Upstream:** ngay TLS syslog cũng không có application-layer ACK; khi connection/session gãy, sender không luôn biết message nào đã đến syslog app phía nhận. [RFC5425, §6.3](https://www.rfc-editor.org/rfc/rfc5425.html#section-6.3).

**Upstream:** legacy RFC3164 là Informational; timestamp kiểu tháng/ngày/giờ thiếu năm và timezone. [RFC3164, §4.1.2](https://www.rfc-editor.org/rfc/rfc3164.html#section-4.1.2).

**Đề xuất:** hỗ trợ RFC5424 payload qua file không đồng nghĩa có syslog listener, TLS/auth, socket limits hay replay. Với listener phải lựa chọn framing explicit hoặc bounded detection, kiểm tra length trước allocation, timeout frame dở, xử lý fragmented reads và nhiều frame/read. Đừng dùng readline cho mọi TCP input. Không coi TLS là exactly-once hoặc durable acceptance. RFC3164 year/timezone inference phải hiện `inferred`, không tạo timestamp giả có vẻ chính xác. Syslog severity cần mapping profile riêng, không reuse số numeric JSON app.

### 6.2 File rotation không được giải bằng một offset đơn lẻ

**Upstream:** GNU tail phân biệt follow descriptor và follow name; `-F` tương đương follow name + retry. Descriptor có thể tiếp tục đọc file đã rename/unlink; name theo pathname mới. Nếu phát hiện file nhỏ đi thì tail báo truncate và bắt đầu lại. [GNU Coreutils manual, tail](https://www.gnu.org/software/coreutils/manual/coreutils.pdf).

**Upstream:** logrotate `copytruncate` copy rồi truncate chính file cũ; tác giả ghi rõ có khoảng thời gian dữ liệu có thể mất giữa hai thao tác. [logrotate upstream manual: copytruncate](https://github.com/logrotate/logrotate/blob/main/logrotate.8.in).

**Upstream:** Fluent Bit có extra rotate-wait để chờ writer flush vào file cũ, offset database và lựa chọn so inode+filename; chính tài liệu Tail cũng thừa nhận copytruncate có race và loss khi pressure. Đây là ví dụ tham khảo, không phải chứng minh TailPro đã cover. [Fluent Bit: Tail](https://docs.fluentbit.io/manual/data-pipeline/inputs/tail).

**Đề xuất:** giữ open descriptor, device/inode/generation và physical byte offset; phát hiện pathname identity thay đổi riêng với descriptor EOF. Nếu yêu cầu drain file cũ cần bounded grace/descriptor budget, đồng thời đọc file mới để tránh bỏ quá lâu; thứ tự hai file phải khai báo. Nếu không hỗ trợ drain thì marker ghi rõ old-tail coverage uncertain. Sau reconnect, file identity/size/guard chỉ tăng confidence, không chứng minh không có truncate-regrow hoặc inode reuse.

**Đề xuất:** copytruncate có hai loại mất khác nhau: reader đọc chậm bỏ bytes trước truncate, và bytes writer ghi trong copy–truncate window có thể biến mất khỏi cả bản copy lẫn bản gốc. Không thuật toán client tail nào khôi phục bytes không còn tồn tại. Có thể giảm xác suất bằng poll/grace/chọn rotation strategy phù hợp, nhưng không gọi đó là zero loss.

## 7. Multiline, buffering và durability: bài học từ collector hiện có

**Upstream:** Fluent Bit phân biệt runtime multiline docker/cri và các parser stack trace; multiline buffer có limit riêng, vượt limit flush với `multiline_truncated=true`. Long-line limit của Tail không tự bound toàn bộ multiline aggregation. [Fluent Bit: Multiline parsing](https://docs.fluentbit.io/manual/data-pipeline/parsers/multiline-parsing).

**Đề xuất:** TailPro cần hai state machines có budget riêng: runtime partial reconstruction và application multiline. Key state ít nhất source + stream + generation + profile version; cap bytes, lines/parts, active groups và age. Idle timeout là heuristic, không bằng chứng stack trace đã đầy đủ. Rotation/disconnect/stop/profile đổi phải flush immutable incomplete part; late lines dùng group link có provenance, không mutate evidence cũ. Không chạy arbitrary indent heuristic toàn source vì request concurrent có thể xen nhau.

**Upstream:** Fluent Bit lưu ý input có thể mất dữ liệu khi memory buffering phải pause. [Fluent Bit: Backpressure](https://docs.fluentbit.io/manual/administration/backpressure). Filesystem queue cũng hữu hạn: `storage.total_limit_size` có thể discard oldest chunk; memory ring buffer cố ý drop oldest và có counters. [Fluent Bit: Buffering](https://docs.fluentbit.io/manual/data-pipeline/buffering).

**Upstream:** Vector phân biệt memory và disk buffers; disk sync theo interval thay vì mọi write. Full buffer có block/drop policy; disk I/O error có thể buộc process exit. [Vector: Buffering model](https://vector.dev/docs/architecture/buffering-model/).

**Upstream:** end-to-end ACK chỉ có durability guarantee khi source tham gia được giao thức ACK; plain socket source không thể yêu cầu sender gửi lại event thất bại. [Vector: End-to-end acknowledgements](https://vector.dev/docs/architecture/end-to-end-acknowledgements/).

**Đề xuất:** local disk spill có thể kéo dài outage window nhưng là feature riêng: quota, free-space reserve, sync policy, recovery, encryption/privacy, schema/version migration, compaction và deletion. Nó không sửa loss trước TailPro, syslog UDP, upstream rotation hoặc thiếu replay token. ACK RAM hiện tại không được đổi tên thành durable ACK. Nếu sau này checkpoint persist, phải commit payload/disposition bền vững trước checkpoint tương ứng; checkpoint đi trước spool tạo loss sau crash, đi sau có thể replay.

**Đề xuất:** pressure ở desktop không nhất thiết truyền ngược an toàn đến writer: SSH pipe có thể block command trong khi source vẫn rotate/drop. Bounded queues + fairness bảo vệ app nhưng không kéo dài retention upstream. Do đó UI/API phải báo pressure/disposition và resume risk, không chỉ queue size.

## 8. Chuẩn hóa theo tinh thần OpenTelemetry, giữ nguồn của từng field

**Upstream:** OTel Log Data Model có Timestamp, ObservedTimestamp, SeverityText/Number, Body, Resource, Attributes và optional TraceId/SpanId/TraceFlags. Timestamp là thời điểm event theo origin clock, có thể thiếu; ObservedTimestamp là lúc collection system quan sát. SeverityNumber tăng theo độ nghiêm trọng. [OpenTelemetry: Logs Data Model](https://opentelemetry.io/docs/specs/otel/logs/data-model/).

**Đề xuất:** không cần triển khai OTLP chỉ để có model rõ. Giữ ba thời gian nếu có: `event_time`, `envelope_time`, `observed_at`; mỗi field đi với `origin`, `parser_profile`, `quality`. Không coi Docker/journal timestamp tự động là app event time. Dùng local ingest sequence cho thứ tự nhận; event time để hiển thị/filter với clock quality, không suy causal ordering liên host.

| Field TailPro đề xuất | Nguồn khả dĩ | Chính sách |
| --- | --- | --- |
| `severity_original` + namespace | App string/number, journal priority, syslog PRI | Giữ nguyên; numeric chỉ diễn giải với known profile |
| `severity_normalized` | Mapping của profile hoặc heuristic | Có provenance; unknown khác INFO; conflict giữ evidence |
| `event_time` | Timestamp app hợp lệ | Numeric cần unit/schema rõ; missing không điền như observed time thật |
| `envelope_time` | Docker/CRI/journal/syslog header | Không overwrite field app; giữ precision và timezone evidence |
| `observed_at` | Clock TailPro lúc nhận | Không gọi đây là latency end-to-end nếu clocks không so được |
| `trace_id`, `span_id` | Explicit payload/context profile | Validate format nếu claim chuẩn; business request ID có namespace riêng |
| `resource/source` | Adapter discovery/identity | Host/unit/container/Pod evidence tách khỏi app user fields |
| `raw`/body + quality | Physical payload/decoded view | Nói rõ bytes nào retained, escaped, truncated hoặc omitted |

**Đề xuất:** parser dựa schema/path cụ thể; không match leaf key mọi depth làm field `context.level` vô tình override `level` gốc. Format nhận diện yếu trả text fallback có `parse_status`, không gán lỗi ingestion. Không suy ERROR chỉ vì dùng stderr hoặc chữ “error” trong JSON field không liên quan. Khi envelope và app severity khác nhau, policy nên deterministic và inspector hiện cả hai.

## 9. Observability cần chứng minh điều gì

Toàn bộ mục này là **đề xuất thiết kế**, không phải metrics đã có trong app.

### 9.1 Counters và gauges theo đúng đơn vị

| Nhóm | Tín hiệu | Câu hỏi giải đáp |
| --- | --- | --- |
| Source | State, last successful metadata/data read, reconnect attempts/reason, exit status, auth/version | Đang idle thật hay reader không còn đọc? |
| Inbound | Bytes nhận, physical frames, bytes/frames bị loại theo reason | Dữ liệu đến đâu trước parser? |
| Decode/parse | Runtime partials, malformed, unknown profile, encoding replacement, oversize | Đọc được nhưng mất fidelity ở tầng nào? |
| Assembly | Open groups/bytes/age, flush reason, incomplete groups | Timeout/cap có chia stack trace? |
| Admission | Accepted events/bytes, explicit rejected/discarded physical ranges | Pressure có gây mất event trước store? |
| Retention | Retained rows/bytes, eviction rows/bytes, pin bytes/age | Evidence còn nằm trong RAM hay đã evict? |
| Checkpoint | Last committed position, outstanding ranges/age, replay classification | Reader đã vượt qua pending record chưa? |
| Coverage | Gap count, known lost bytes/records, unknown-loss intervals, scope changes | Không thấy event có đủ cơ sở kết luận không? |
| Delivery | Query/export pagination/truncation/expiry, redaction transforms | Kết quả downstream có đầy đủ trong scope đã xin? |

Không cộng physical lines với normalized events: một stack trace dùng nhiều frames và một frame có thể mang nhiều records theo protocol. Chỉ viết conservation invariant trên cùng đơn vị và cùng generation/fence. Counters lifetime không reset lặng khi source restart; generation/scope thay đổi có marker. Labels bounded theo source handle/reason enum, không đưa message/raw path/secret vào metric label.

### 9.2 Coverage là dữ liệu, không chỉ một banner

`complete=true` nên có nghĩa *đã duyệt xong phạm vi retained snapshot được khai báo, không biết có gap trong phạm vi đó*, không có nghĩa tất cả log app từng phát đều tồn tại. Tách `query_complete`, `retention_coverage`, `source_continuity` và `payload_fidelity`. Có thể query xong nhưng source continuity unknown; raw còn retained nhưng normalized field unknown.

Gap/disposition có source generation, physical position/range nếu biết, observed interval, reason, certainty và count đơn vị rõ. Không invent số lượng mất khi chỉ biết disconnect interval. `lost_count=null` tốt hơn số 0 giả. Marker không được biến mất khỏi coverage chỉ vì user filter `level:error` không match text marker.

UI, MCP và export phải cùng truyền coverage/time basis. Agent không nên coi không thấy request sau deploy là evidence fix thành công nếu cursor hết hạn, retention evict, source mất kết nối hoặc query truncated. Quyền/redaction áp dụng cho quality/provenance metadata cũng như body để không rò nguồn ngoài scope.

### 9.3 SLO và bằng chứng có thể công bố

Tách retrieval lag, local queue age, parser/assembly delay và UI publication delay. `observed_at - event_time` chỉ là estimated age khi clocks/time semantics tin cậy; wall clock có thể nhảy. Dùng monotonic local clock cho durations nội bộ. Quiet source không tự bị gắn stale chỉ vì không có event mới; lấy control health/read success làm evidence riêng.

Mỗi phép đo lưu source count, bytes/record distribution, multiline ratio, transport RTT/bandwidth, host/daemon versions, retention policy, initial backlog, duration, output scope, pressure dispositions và process-tree RSS. “Zero drop trong mẫu có sequence N trong điều kiện X” là kết quả kiểm tra; không biến thành “zero drop với mọi VPS”. Không có throughput number mới nào được suy từ tài liệu collector trong báo cáo này.

## 10. Acceptance cần thiết trước khi nâng mức cam kết

Đây là **đề xuất matrix kiểm tra thủ công**, tuân [AGENTS.md](../../AGENTS.md) không thêm automated tests.

| Tình huống | Evidence cần quan sát |
| --- | --- |
| App JSON nằm trong Docker/journal/CRI | Envelope riêng, app fields đúng path/unit, raw fallback và quality chính xác |
| Equal timestamps + identical legitimate repeats | Không hard-suppress repeat hợp lệ; replay ambiguity hiện rõ |
| Reconnect khi pending multiline chưa ACK | Checkpoint không vượt physical prefix chưa disposition; stale generation ACK bị bỏ |
| Long line + deep/malformed JSON + binary/repeated journal fields | Bounded allocation; số bytes omitted/truncated đã biết; không báo fidelity complete giả |
| Rename/create + writer tiếp tục append file cũ | Old/new identity, grace/drain policy, marker và coverage đúng |
| Copytruncate rồi regrow trước poll | Detect được thì uncertain/reset; không chứng nhận exactly-once khi guard không chứng minh được |
| Docker non-blocking/cache full/driver failure | Phân biệt upstream risk với local accepted/discarded; không claim collector đo được mọi upstream drop |
| Journal vacuum exact checkpoint | Exact match verified hoặc explicit uncertain fallback; không dựa exit 0 |
| Kubernetes nhiều restart/rotation/eviction | Latest-file/previous giới hạn đúng; Pod UID/attempt không reuse nhầm |
| Slow consumer/paused UI/full budgets | App responsive, fair source admission, counters/gaps không mất; retention eviction tách pressure drop |
| Stop/disconnect/profile edit | Group flush đúng lý do, cancellation/retry tree dừng, checkpoint generation fence |
| Query/export qua gaps + redaction | Metadata coverage tồn tại dù filter không match marker, không lộ unauthorized source |

## 11. Phần chưa chốt và quyết định nên giữ mở

1. **Không suy universal compatibility từ số adapter.** Cần chọn workload/host/version thực và publish matrix validated/partial/unavailable; Kubernetes và network syslog là capability mới, chưa được nghiên cứu này xác nhận có trong TailPro.
2. **Cursor journal cần implementation probe riêng.** Semantics thư viện đã rõ, nhưng CLI exact-cursor validation/fallback và quyền đổi cần chạy trên systemd target. Latest docs không chứng minh host Ubuntu cụ thể có mọi option.
3. **Kubernetes source có comment stale.** Dùng docs latest-file-only làm giới hạn lịch sử; rotation-follow thực tế cần versioned source và scenario test, không lấy một comment làm chân lý.
4. **Docker timestamp boundary không thành unique ID.** Cần đo daemon/driver mục tiêu; giữ conservative replay và `uncertain` nếu không đủ evidence. TTY và CLI diagnostics làm stream fidelity giảm.
5. **Raw fidelity là quyết định sản phẩm.** Escape/lossy UTF-8/truncation/binary omission phải explicit. Bounded byte retention tăng độ tin cậy forensic nhưng cần budget, redaction và UI safe rendering; không tự bỏ cap để “100%”.
6. **Durability phải có scope riêng.** RAM retention/checkpoints hiện tại không chịu app crash/restart. Spool có sync/quotas/recovery là dự án khác; disk hữu hạn vẫn cần full policy và upstream replay contract.
7. **Custom parser/multiline cần provenance/version.** Thay profile không được diễn giải lại checkpoint/partials đang dở như cùng generation; rule fallback phải giữ evidence và CPU/memory bounds.

## 12. Nghiên cứu bổ sung: tối ưu parser/search mà giữ fidelity

### 12.1 Dữ liệu có kiểu và namespace quan trọng hơn số lượng alias

**Upstream:** Pino có các level số10/20/30/40/50/60 và hỗ trợ custom levels. Timestamp mặc định theo milliseconds, có helpers xuất seconds hoặc ISO/RFC3339. Vì vậy numeric `level` hoặc `time` không có một cách giải mã universal. [Pino API](https://raw.githubusercontent.com/pinojs/pino/main/docs/api.md).

**Upstream:** Redis code ánh xạ các mức debug/verbose/notice/warning qua chuỗi ký hiệu `.-*#`, kèm PID/role/date header. `.` và `-` không cùng level. [Redis server logging implementation](https://github.com/redis/redis/blob/unstable/src/server.c).

**Upstream:** Nginx cho phép `log_format` tùy ý; `$status` là field response status trong schema log được cấu hình. Combined format đặt status sau quoted request. Không có guarantee rằng số500 bất kỳ trong message là HTTP status. [Nginx log module](https://nginx.org/en/docs/http/ngx_http_log_module.html).

**Đề xuất:** một mapper chung có typed alias/path, numeric namespace/unit, deterministic precedence và conflict evidence. Redis/HTTP là profile khai báo có điều kiện header, không chạy symbol/status regex toàn message. Giữ Unknown khi không đủ bằng chứng. Timestamp nanosecond lớn cần giữ integer/decimal literal, không qua floating point rồi gọi chính xác tuyệt đối.

**Upstream:** reference decoder logfmt phân biệt key, value trống, quoted value và escaping. [go-logfmt decoder](https://raw.githubusercontent.com/go-logfmt/logfmt/main/decode.go).

**Đề xuất:** dùng bounded state machine cho logfmt; whitespace splitting không xử lý đúng quoted message. Corpus phải có malformed suffix/duplicate keys/escaped quotes; tốc độ không được đổi lấy field extraction sai.

### 12.2 Giảm dữ liệu materialize trước khi đổi parser

**Upstream:** Serde JSON `RawValue` biểu diễn JSON range và có borrowed form; có thể giữ dữ liệu con mà không dựng cả `Value` tree. [Serde JSON RawValue](https://docs.rs/serde_json/latest/serde_json/value/struct.RawValue.html).

**Đề xuất từ code repo:** TailPro đã có selective JSON visitor và lazy Inspector; mở rộng đường này trước khi thay bằng DOM/parser khác. Hiện normalizer tạo raw String, ANSI-stripped String và field String; store giữ raw/message riêng. Đo allocations/copied bytes và thử raw spans, decoded overlay khi thực sự cần, shared source identity. Full JSON validation và escaped-field decode vẫn có chi phí; borrowed không đồng nghĩa mọi record zero allocation.

**Upstream:** `memmem::Finder` cho phép compile searcher một lần để tìm cùng needle qua nhiều byte slices. [memchr memmem](https://docs.rs/memchr/latest/memchr/memmem/index.html).

**Đề xuất:** benchmark với `.contains()` hiện tại trên short/long needle, short/large log và local/sanitized paths. Tái sử dụng compiled query/searcher là giả thuyết có cơ sở; chưa có số đo để claim SIMD tăng tốc bao nhiêu lần. Không cần index trước khi linear scan vượt target thực tế.

**Upstream:** simd-json có SIMD runtime selection trên x86 khi bật feature tương ứng và fallback khi CPU không hỗ trợ; thư viện có các API/representation khác nhau. [simd-json](https://docs.rs/simd-json/latest/simd_json/).

**Đề xuất:** so chi phí parse + scratch/copy + selected output + raw retention + query + RSS, cùng full validation. Throughput DOM microbenchmark không trả lời được selective engine có nhanh hơn không. Batching bounded có thể giảm dispatch/ACK overhead nhưng cần đo quiet-source latency, fairness, cancellation và first-event delay; không tăng concurrency vô hạn.

### 12.3 Regex cấu hình và fragmentation có chi phí an toàn riêng

**Upstream:** Rust regex hỗ trợ named groups, không hỗ trợ lookaround/backreferences, có size limits. Single-search complexity bị chặn theo pattern/input size; match iterators có thể có worst-case bậc hai theo input. [Regex documentation](https://docs.rs/regex/latest/regex/).

**Đề xuất:** cap số rule, pattern bytes, compiled size, input và số candidate; compile một lần theo revision, một capture search mỗi candidate. Linear-time theo input không đủ nếu pattern rất lớn hoặc chạy hàng trăm rule mỗi dòng. Không dùng “có timeout” để giả định CPU job bị kill ngay lập tức.

**Đối chiếu repo:** `redaction::project` sanitize từng event raw; query literal match từng event message/raw. Fragmentation vì vậy là thay đổi xuyên parser/store/query/policy, không chỉ framer.

**Đề xuất:** local literal search cần state bounded qua fragments contiguous còn retained, tránh miss needle ở boundary hoặc join giant record. External projection phải sanitize ở logical-record context trong budget, hoặc fail closed với oversize/incomplete/orphan fragments: secret bị split không được để suffix lọt vì mất key ở phần đầu. Không count/search trên raw secret trước policy. Parent/checkpoint metadata cần coalesced ranges để một dòng không LF lớn hơn retention không tạo vô hạn pending entries; resume file offset khác Docker timestamp và Journal cursor.

Kết quả research được cụ thể hóa trong [plan phase6](../plan/phase-06-universal-log-ingestion.md). Chưa thay code hoặc trạng thái runtime acceptance. Các giới hạn upstream là lý do cần kiểm chứng đúng môi trường, không phải bằng chứng TailPro đã thất bại hoặc đã đạt các gate đó.
