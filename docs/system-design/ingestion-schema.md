# Ingestion schema v1 — contract mục 6.1

Chốt ngày **25/11/2026, Asia/Bangkok**. Schema nội bộ version1 độc lập với desktop/MCP wire version2. Phần6.1 ghi contract/foundations tại thời điểm chốt. Migration6.2–6.8 hiện có trong runtime; xem mục8 và acceptance để phân biệt lịch sử6.1 với capability mới. Không gọi schema đã khai báo là runtime đã hỗ trợ format mới.

Mã chuẩn: [`model/ingestion.rs`](../../crates/log-core/src/model/ingestion.rs). Admission outcome hiện hành dùng chung type này qua re-export `engine::Disposition`. [Plan](../plan/phase-06-universal-log-ingestion.md), [evidence](../process/phase-06-acceptance.md) và [contracts](contracts.md) ghi phạm vi triển khai.

## 1. Identity, đơn vị dữ liệu và ownership

| Khái niệm | Contract |
| --- | --- |
| Source handle | Opaque, scoped trong app session và quyền đã cấp; pathname/Pod name không tự làm stable identity |
| Reader generation | Fence cho ACK/cancel/reconnect theo reader lifecycle; ACK khác generation không được cập nhật checkpoint |
| Content generation | File rotation/truncate hoặc container instance/boot thay đổi; khác reader generation; không nối multiline/fragment qua boundary |
| Stream | Giữ nguồn stdout/stderr/file/journal; TTY chưa xác minh không được tự tạo hai stream; stream không quyết định severity |
| Event ID | Opaque retained reference của engine; không dùng timestamp/hash message để dedupe hoặc chứng minh exactly-once |
| Logical record | Record của payload sau khi bỏ envelope đã định nghĩa; có thể gồm nhiều bounded fragments |
| Admission unit | Một atomic record/fragment được giữ đầy đủ theo representation đã khai báo hoặc có gap disposition; không phải lúc nào cũng là một event |
| Physical IDs hiện hành | Source-local sequence, `Event.physical`≤200; checkpoint pending≤256; không đổi ý nghĩa counter cũ khi chưa migrate |

Record lineage của schema mới cần `(source handle, reader generation, content generation, stream, logical record identity)`. Fragment thêm raw range và final/incomplete state; byte ranges liền nhau không đủ chứng minh cùng record nếu lineage khác. `PayloadRange::followed_by` chỉ kiểm tra adjacency; caller phải kiểm tra lineage trước.

Payload nằm ở reader/retained segments; borrowed view không sở hữu buffer và không kéo dài lifetime. Chỉ publish immutable owned/leased data; không giữ borrowed reference vào scratch buffer sau khi recycle. Không thêm lifetime-erasing pointers, `unsafe`, cache hay pool riêng cho schema. Dynamic strings/decoded overlays/link metadata phải charge theo ledger hiện hành khi các bước sau thêm vào runtime.

## 2. Raw fidelity và byte ranges

`PayloadRange=[start,end)` là zero-based raw **payload byte range trong logical record**, có thể rỗng. Nó không phải file offset, character index, Docker timestamp, Journal cursor hoặc event ID. Constructor từ chối end<start; phép trừ không underflow. `OriginalPayload` chỉ nhận range có độ dài đúng số bytes và không vượt byte limit của caller cũng như ceiling256KiB. Docker/Journal caller còn phải đặt cap nhỏ hơn theo working budget hiện hành.

`OriginalPayload` borrow bytes, giữ invalid UTF-8/NUL/CR nguyên bản; không tạo display String. Nó không derive Serialize để tránh vô tình dùng raw làm external DTO. Điều này không thay application authorization/redaction policy. Runtime hiện tại vẫn giữ lossy String; chỉ đường6.2 mới chuyển ownership sang original bytes/fragments.

Mỗi adapter phải khai báo riêng:

- Envelope/raw boundary: byte nào là payload, byte nào là timestamp/header/stream metadata; chỉ cam kết fidelity của bytes reader đã quan sát, không của mọi bytes app từng ghi.
- Terminator: giữ loại/kích thước LF, CRLF, EOF hoặc interrupted/unknown; không reconstruct synthetic LF rồi gọi byte-identical. LF framing hiện tại giữ CR trong frame, bỏ LF; khi migrate phải ghi nhận thay đổi này rõ.
- Representation: original bytes, decoded text, reconstructed envelope value, placeholder/omitted, truncated. Payload fidelity và completeness là hai trục độc lập, không dùng `Accepted` thay cho fidelity.
- Range coverage: represented contiguous range, known missing range và unknown loss; retained coverage thay đổi khi eviction dù admission trước đó thành công.

Oversize chuyển raw-fragment mode sau cap, không parse từng mảnh JSON như record độc lập. Một dòng không LF có thể dài hơn retention: emit fragments hữu hạn mỗi lượt, giữ parent summary/ranges coalesced có cap; không giữ danh sách mọi fragment đã đi qua. UInt offset/sequence overflow phải explicit error/gap/NeedsAction, không wrap về0.

Local literal search nối state qua contiguous retained fragments trong cùng lineage, bộ nhớ phụ thuộc chiều dài query đã cap; không join giant record, không rescan prefix dẫn tới O(n²). Mất neighbor trả incomplete. External search/export/AI chỉ dùng logical-record sanitized projection trong budget; incomplete/oversize/orphan phải mask/withhold trước predicates/counts để secret suffix không lọt qua boundary. Các hành vi này là gate6.2/6.8, chưa có từ primitives6.1.

## 3. Time và provenance

| Field | Ngữ nghĩa | Chọn để display |
| --- | --- | --- |
| `event` | Thời điểm app event đã resolve được epoch, unit/timezone hợp lệ | Ưu tiên1 |
| `transport` | Thời điểm envelope/runtime/journal báo, cùng semantics cụ thể | Ưu tiên2 |
| `observed` | Clock của TailPro lúc quan sát, không phải timestamp app được suy đoán | Fallback cuối |

`EventTimes::display()` trả cả value và `TimeBasis`; không overwrite một clock bằng clock khác. Candidate literal chưa resolve chỉ giữ original evidence, không điền thành event timestamp giả. Display order khác ingest sequence: engine vẫn xếp theo local admission order; liên host không có causal proof từ timestamp.

`UnixNanos` là decimal **string** có giá trị trong signed128-bit nanoseconds. Constructor chỉ nhận canonical integer (`0`, positive không leading zero, negative không `-0`), từ chối plus sign/decimal/exponent/whitespace/overflow. Serialize luôn JSON string kể cả giá trị lớn hơn2^53; không chuyển qua float. Signed pre-epoch cho phép dữ liệu trước1970; đây không phải tuyên bố tương thích binary OTLP timestamp.

Source timestamp parser ở6.3 phải parse units/decimal parts bằng checked integer arithmetic, tạo canonical ns text có bounded ownership. Không coi unit nanos là precision: literal `.123` vẫn chỉ có precision3 digits dù normalized ns thêm zero. Original field evidence phải giữ:

| Metadata | Giá trị/giới hạn |
| --- | --- |
| Location | Root/nested path đã chọn hoặc raw span trong record; không recursive-match leaf bất kỳ |
| Original literal | Giữ qua raw span nếu được; nếu decode cần allocation thì charge; không copy cả payload vào metadata |
| Unit/precision | Explicit seconds/ms/us/ns; fractional digits0–9 hoặc unknown; mơ hồ không auto-resolve |
| Timezone | Offset trong literal, configured source timezone, heuristic hoặc unknown; thiếu year/DST giữ ambiguity |
| Origin | `explicit`, `profile`, `heuristic`, `unknown` từ `FieldOrigin`; không probability số giả |
| Conflict/rule | Bounded reason enum, selected alias, decoder/profile revision; chưa có profile thì không fabricate revision |

`FieldOrigin::Explicit` chỉ nói field được ghi rõ, không chứng minh app nói đúng hay giá trị hợp lệ. Validate timestamp/calendar/type độc lập. Envelope provenance không lấy từ app field giả tên host/source/cursor.

## 4. Severity và mapping contract

Giữ `Level` hiện hành cho display/filter, bao gồm Unknown và Error gộp fatal. Giữ original text/numeric literal cùng namespace/provenance riêng để không mất fatal/critical distinction. `SeverityNamespace` có `text`, `pino`, `syslog`, `open_telemetry`, `custom_profile`, `unknown`; enum không tự detect hay map numeric level.

Numeric value thiếu namespace/profile giữ Unknown. Namespace khác nhau không share mapping table. Custom namespace bắt buộc profile ID/revision hợp lệ; không nhận alias profile từ raw app làm trusted configuration.

Priority chốt cho mapper6.3–6.4: explicit selected profile path → valid root aliases theo thứ tự cố định → declared nested paths → bounded header heuristic → Unknown. Time root aliases: `@timestamp`, `timestamp`, `time`, `ts`. Severity root aliases: `level`, `lvl`, `severity`, `severity_text`; known-namespace numeric fields xử lý theo profile. Message: `message`, `msg`, rồi declared `error.message`/`err.message` khi thiếu message.

First duplicate occurrence của **cùng một path** thắng, kèm duplicate conflict flag; priority giữa các alias không phụ thuộc thứ tự JSON key. Nếu alias ưu tiên cao invalid, chọn alias hợp lệ tiếp theo và ghi rejected-candidate reason. Conflict giữa root và unrelated nested field không được làm thay đổi selection. Schema/profile migration không sửa semantics event đã publish; thay rule phải fence partial state.

Đây là contract mapping đã chốt, **chưa là hành vi normalizer hiện tại**: walkthrough cho thấy nested-before-root có thể đổi Info thành Error và numeric time chưa được lấy. Các lỗi này cần sửa tại6.3/6.4 trước khi claim mapping compliant.

## 5. Disposition, checkpoint và accounting

`Disposition::{Accepted,Gap}` là terminal outcome hiện hành, được chuyển về model nhưng giữ public path `engine::Disposition` và signature `Ack` để không phá callers. `Accepted` chỉ chứng minh admitted vào RAM tại thời điểm ACK; `Gap` là dữ liệu đã có explicit loss disposition. Cả hai không chứng minh durable storage hoặc fidelity complete.

Adapter chỉ tiến checkpoint qua **contiguous terminal prefix trong cùng reader generation**. Group nhiều physical IDs chỉ ACK khi admission/disposition của group đã xong. Khi tiến qua Gap, coverage ledger/source-loss state phải ghi thiếu, không để marker bị filter hoặc eviction làm coverage thành complete giả.

Schema fragmented mới cần counters khác counter legacy:

```text
admission_units_total = accepted_units_total + gap_units_total + inflight_units
observed_payload_bytes = represented_payload_bytes + explicit_gap_bytes + inflight_payload_bytes
```

`represented_payload_bytes` cumulative tại admission; resident/retained bytes là gauge sau eviction. Một logical record gồm cả accepted/gap units là `partial_with_gap`, không tăng cả accepted/gap cho cùng unit. Physical-line counter tăng khi record delimiter/EOF được xác nhận, không theo số fragments; replay là quan sát thêm, không silently dedupe. Unknown upstream bytes/lost records giữ unknown, không điền0. Các phương trình mới chưa được dùng để diễn giải lại counters runtime cũ.

| Adapter | ACK/resume boundary khi chuyển sang fragments | Hạn chế |
| --- | --- | --- |
| File | Verified absolute next-byte offset + identity/guard và partial-record continuation state; payload range không thay offset | Resume mid-record cần lưu record lineage/assembler state gọn hoặc báo orphan/uncertain; cap chưa có implementation6.2 |
| Docker | Committed complete timestamp boundary; giữ last complete checkpoint khi record còn dở | Không có fragment cursor; replay overlap/uncertainty. Coalesced parent giữ O(1) range summary, không một pending entry mỗi fragment |
| Journal | Opaque entry cursor sau full representation hoặc explicit terminal entry gap | Không ACK một cursor chỉ vì prefix JSON/entry đã đọc; nếu quá frame cap mà không recover cursor/entry boundary an toàn thì giữ checkpoint cũ và NeedsAction/uncertain |

Stop/cancel trước terminal disposition không tự biến thành Accepted. Flush partial data nếu policy/budget cho phép, hoặc ghi explicit gap khi biết range; nếu chưa quyết định được thì giữ cursor trước đó và continuity unknown. Reconnect không kế thừa pending state bằng generation giả. Các thay đổi fragment ACK cần giữ ledger≤256 entries/source và source working cap, được triển khai ở6.2/6.9.

## 6. Source capability matrix đã chốt

Capability mô tả **cách adapter đọc**, không chứng nhận environment/version/quyền thực. Trạng thái capability gồm implemented-unverified/observed/unsupported; permission/history/stream fidelity có thể vẫn unknown khi stream đang chạy.

| Adapter | Identity / resume | History/fidelity baseline | Trạng thái TailPro |
| --- | --- | --- | --- |
| File | Descriptor device/inode/content generation; raw-byte next offset/guard | Initial recent window; rotation/copytruncate có uncertainty; original-byte retention chưa có | Reader có code/probes; target runtime gates partial |
| Docker/Compose | Exact container ID; timestamp overlap | Driver/cache/TTY/retention phụ thuộc target; CLI stderr cần phân biệt diagnostics | Reader có code/stub evidence; daemon matrix chưa accepted |
| Journal | Selected unit + opaque cursor; exact-cursor validation | Visible system journal only; binary/repeated/large MESSAGE hiện còn mất fidelity | Reader có code/helper probes; daemon/vacuum/quyền thật chưa accepted |
| Kubernetes | Context/namespace/Pod UID/container instance; timestamp overlap | Latest-file/previous/history limits, scoped auth/context trust riêng | Unsupported ở registry hiện tại; gate6.12 |
| Direct syslog/OTLP/cloud APIs/Kafka topics | Chưa có adapter contract triển khai | Đọc syslog text trong file không phải socket receiver; broker log không phải topic | Unsupported; backlog riêng |

Không thêm capability field vào desktop/MCP ở6.1. Publication vào SourceInfo/read responses thuộc6.8/6.12 và phải qua grants/redaction, không lộ metadata của nguồn ngoài scope.

## 7. Compatibility và next step

- `Event`, field layout, parser/time overwrite behavior, frame caps, checkpoint runtime, UI/MCP payloads vẫn là legacy. Primitive views không thêm per-event storage hoặc đổi `charged_bytes()`.
- Schema constants không thay `app_protocol::WIRE_VERSION=2`; chưa sửa wire DTO nên không regenerate TypeScript. Khi6.2/6.3 đưa schema vào runtime, update generated contracts nếu wire thực sự đổi; toàn bộ public integers lớn vẫn strings.
- `OriginalPayload`, `UnixNanos`, `EventTimes` là foundations cho6.2/6.3; chưa có decoder, storage migration, new diagnostics endpoint hoặc telemetry export trong6.1.
- Next scope đề xuất **6.2**: bounded original-byte/fragments, identity/range/ACK migration và fail-closed external projection. Chưa bật fragmented payload cho MCP/export trước khi boundary-redaction gate có evidence.

## 8. Runtime migration6.2–6.8, cập nhật25/11/2026

Các câu “runtime hiện tại/legacy/chưa migrate” ở mục1–7 là snapshot foundations6.1. Runtime mới dùng `RawPayload` với Vec original bytes và optional lossy display overlay; LF/CRLF/EOF/interrupted riêng, fragments có record/range/final lineage. File checkpoint có partial continuation; Docker/Kubernetes giữ complete parent boundary; Journal binary/repeated MESSAGE có bounded parts và entry ACK. Normalizer dùng deterministic declared paths, full JSON validation/logfmt grammar/header profiles; event/transport clocks tách riêng. Exact normalized ns trong evidence và sanitized JSON numbers là strings/arbitrary-precision literals, không JS Number/f64 round-trip.

Inspector có original hex/delimiter/lineage, local search qua contiguous fragments, external projection mask fragmented/incomplete/lossy bodies trước predicates. Source counters tách admission/retention; legacy counters giữ tên cũ. Desktop/MCP schema2 có additive evidence/coverage/counter fields, TS regenerate từ Rust. [Config v1](parser-configuration.md) giới hạn rõ rule/profile được chấp nhận; không có arbitrary plugin/DST resolver/remote configuration loading. [Acceptance phase6](../process/phase-06-acceptance.md) có observations; runtime Gates A/B/C còn Partial/Unrun/Deferred.
