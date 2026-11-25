# Nghiên cứu UX debugging đơn service và microservice

Ngày đối chiếu: 25/11/2026. Đây là findings và đề xuất để thảo luận; không thay đổi contract, roadmap hoặc trạng thái acceptance. Nguồn ngoài repo chỉ dùng tiêu chuẩn và tài liệu chính thức. Tài liệu Jaeger được ghim 2.23; các trang `latest` khác cần kiểm tra lại khi triển khai.

**Đề xuất chính:** TailPro nên giúp người dùng đi từ một lỗi đến đúng nhóm log, rồi theo ID qua các nguồn đã chọn; chỉ mở waterfall phân tán khi có dữ liệu span thực. Cùng một luồng thao tác dùng được cho một service có nhiều file/worker lẫn nhiều service trên nhiều host. Trace và nền tảng observability là khả năng bổ sung, không phải điều kiện để bắt đầu đọc log SSH.

**Ưu tiên sản phẩm theo yêu cầu người dùng:** Thiết kế mục tiêu lấy công việc vận hành thực tế làm chuẩn, không bị giới hạn bởi UI, source cap hoặc kiến trúc code hiện tại. Phần kiểm kê code dưới đây chỉ phục vụ ước lượng triển khai; không phải tiêu chí thu hẹp trải nghiệm. Hai chiều phải tách riêng: hệ thống đơn/microservice và dữ liệu không ID/có ID/có spans. Một app đơn vẫn có thể có tracing HTTP/DB/worker; một hệ microservice vẫn cần dùng tốt khi chỉ có legacy logs. Không đặt chế độ người dùng bằng tên kiến trúc backend.

## 1. TailPro hiện có gì

| Quan sát từ code/contract | Hệ quả cho thiết kế |
| --- | --- |
| `main.tsx` truyền `scope` của log viewer vào Trace; `scope` có thể là các nguồn của host đang chọn. Trace dùng `sources` được truyền, chỉ fallback sang toàn bộ `status.sources` khi danh sách rỗng. [App](../../src/app/main.tsx), [Trace](../../src/features/trace/Trace.tsx) | Đang xem một host rồi mở Trace có thể chỉ tìm trên host đó. Cần cho thấy scope trước khi người dùng hiểu kết quả là xuyên hệ thống. |
| Trace gửi exact `trace_id` hoặc `order_id`, cửa sổ theo `observed_at_ms`, tối đa 200 events/128 KiB mỗi lần đọc UI; backend match trên redacted projection. [Trace](../../src/features/trace/Trace.tsx), [agent_context](../../crates/app-core/src/application/agent_context.rs) | Đây là tìm evidence còn retained theo ID, chưa phải truy vấn distributed spans. UI hiện chưa tự đi tiếp `next_cursor`. |
| Timeline hiện có marker hoặc thanh so sánh `duration_ms` explicit; không đặt thanh theo thời điểm bắt đầu causal. DTO `TracePoint` không mang cây parent/link spans. [Trace](../../src/features/trace/Trace.tsx), [output DTO](../../crates/app-protocol/src/output.rs) | Không gọi thanh hiện tại là waterfall đo latency xuyên host. Missing parent/clock offset vẫn unknown. |
| Runtime chặn khi đã có 8 sources; request read cũng có guard 8. Đây là cap nguồn đọc, không phải 8 service logic. [Application](../../crates/app-core/src/application/mod.rs), [read validation](../../crates/app-core/src/application/agent_context.rs) | Một service với access/error/app/worker logs đã có thể chiếm nhiều nguồn. Nhiều replica làm tăng số nguồn nhanh; không chỉ đổi con số 8 để coi là scale được. |
| SourceSpec hiện có File, Docker, Journal, Kubernetes; không có nguồn OTLP span. Kiến trúc hiện tại không có ingestion backend hoặc tracing agent cài trên server. [SourceSpec](../../crates/app-protocol/src/desktop.rs), [architecture](architecture.md) | Instrumentation OTel của app và nhận/truy vấn spans trong TailPro là hai việc riêng; hiện chưa có native OTLP span ingestion. |
| Service mappings gộp tên service với repo label/root bắt buộc; repo chỉ là khai báo, không được mở/index. [Mappings UI](../../src/features/settings/ServiceMappings.tsx), [validation](../../crates/app-core/src/application/production.rs) | Nên tách nhận diện service khỏi mapping local code; người vận hành không có checkout vẫn phải nhóm log theo service được. |

Phase 4 đã có code nhưng gate vẫn PARTIAL. Các tiêu chí matched start/end, missing parent và causal waterfall trong kế hoạch không có nghĩa runtime hiện đã hỗ trợ đầy đủ. [Phase 4](../plan/phase-04-production-debugging.md), [acceptance](../process/phase-04-acceptance.md).

## 2. Bằng chứng từ hệ sinh thái

| Nguồn chính thức | Điều đã xác nhận | Áp dụng vào TailPro — suy luận thiết kế |
| --- | --- | --- |
| [W3C Trace Context, §3.2](https://www.w3.org/TR/2021/REC-trace-context-1-20211123/#traceparent-header), [OTel propagation](https://opentelemetry.io/docs/concepts/context-propagation/) | HTTP `traceparent` truyền trace ID và parent ID; service nhận tạo span cùng trace với quan hệ parent. SDK có thể đưa trace/span context vào log. | ID dùng tìm log; parent/span context và span data mới hỗ trợ dựng quan hệ request có bằng chứng. |
| [OTel Logs Data Model](https://opentelemetry.io/docs/specs/otel/logs/data-model/#trace-context-fields) | Log có thể mang `TraceId`, `SpanId`; đây là field optional. Timestamp gốc khác thời điểm hệ thu thập quan sát. | Không yêu cầu mọi log có ID; giữ nguồn clock và cách correlation trên từng kết quả. |
| [Grafana trace ↔ logs](https://grafana.com/docs/grafana/latest/datasources/tempo/configure-tempo-data-source/configure-trace-to-logs/) | Điều hướng hai chiều cần cấu hình cả log và trace data source; query dùng window, tags, trace/span ID. | Một click từ field ID sang request context; click event/span mở log kề bên. Phân biệt đúng ID với log chỉ cùng service/time. |
| [Jaeger 2.23 Features](https://www.jaegertracing.io/docs/2.23/features/) | Trace view có Gantt, chọn span mở side panel; có graph, flame graph, statistics và JSON. | Chọn một timeline đơn giản với Inspector; nhiều kiểu graph không cần là UI mặc định. |
| [Datadog Trace View](https://docs.datadoghq.com/tracing/trace_explorer/trace_view/), [OTel logs correlation](https://docs.datadoghq.com/opentelemetry/correlate/logs_and_traces/) | Có Flame Graph/Span List/Waterfall/Map, span search, chi tiết span; log có thể mở trace, span có thể mở logs. | Giữ đường đi lỗi → context → request → evidence; search/highlight service hoặc lỗi trong một trace trước khi thêm dashboard. |
| [OTel Link API](https://opentelemetry.io/docs/specs/otel/trace/api/#link), [Datadog Span Links](https://docs.datadoghq.com/tracing/trace_collection/span_links/) | Span links tham chiếu span trong cùng hoặc khác trace; có ích cho async, batch và fan-in. | Quan hệ async là link có loại rõ ràng; không ép tất cả vào một cây HTTP request. |

## 3. Các trường hợp thực tế phải hỗ trợ

Bảng sau là **kịch bản và yêu cầu đề xuất**, không phải khẳng định TailPro hiện đã thực hiện tất cả.

| Bối cảnh | Người dùng cần làm | Evidence có thể kết luận |
| --- | --- | --- |
| Một service, một máy | Chọn app/access/error files, stdout/stderr và worker; tìm error, mở multiline stack và các dòng quanh lỗi. | Ngữ cảnh cục bộ cùng source/process; chưa mặc định là một request nếu nhiều request xen kẽ. |
| Một service, nhiều process/replica | Nhóm theo service, mở replica phát lỗi, đối chiếu các replica trong cùng window. | Service logic giống nhau không có nghĩa cùng process, host hoặc cùng request. |
| Nhiều service HTTP/gRPC | Tìm một request trên đúng môi trường và tất cả nguồn liên quan; xem service/instance nào có evidence. | Shared ID nối tập log; span parent context nối các operation nếu có span data. |
| API → queue → worker | Theo job/message, thấy retry/attempt, nhiều consumer hoặc batch. | Cần propagation qua message và link/parent được instrumentation ghi; không suy queue latency từ hai log gần nhau. |
| Legacy logs không có ID chung | Thu hẹp service, window, literal message/path; so sánh context thủ công. | Chỉ là log có liên quan theo điều kiện tìm; không vẽ request path hoặc báo đã theo toàn bộ request. |

OTel định nghĩa service là thành phần logic, instance là bản chạy riêng; các replica giữ cùng `service.name`. [Service semantic conventions](https://opentelemetry.io/docs/specs/semconv/resource/service/). Vì vậy đề xuất tách các chiều: project/namespace, service, environment, cluster, instance/process/container, host, source và stream. Host là nơi chạy; source là đầu đọc; stdout/stderr là stream; chúng không thay thế service identity. Mỗi mapping cần provenance `user-declared` hoặc metadata đã xác thực; thiếu dữ liệu giữ unknown.

`deployment.environment.name` là chiều deployment riêng trong OTel, không làm thay đổi uniqueness của service theo tiêu chuẩn. [Deployment conventions](https://opentelemetry.io/docs/specs/semconv/resource/deployment-environment/). TailPro vẫn nên luôn đưa environment/cluster vào **scope điều tra** để tránh lẫn staging với production; không diễn giải scope ứng dụng này thành quy tắc identity mới của OTel. Mapping tới repo/revision là optional attachment, tách khỏi service/source mapping.

## 4. Ba mức dữ liệu, ba mức lời hứa

1. **Không có common ID:** log explorer và context theo source/time/query. Có thể hiển thị các lane để đối chiếu; không có causal edges. Đây là fallback thiết kế bắt buộc cho legacy logs.
2. **Có shared correlation ID:** exact lookup trên các nguồn được phép, nhóm theo service/instance và giữ original evidence. `request_id`, `order_id`, `job_id` cần typed namespace và scope; hỗ trợ field/profile rõ thay vì coi chúng tự động đồng nghĩa. Một order có thể liên quan nhiều lần API, retry, job và trace — đó là mô hình nghiệp vụ cần người dùng/app khai báo, không phải quy tắc của trace ID. Contract hiện cũng cấm coi cùng business ID là cùng request. [Correlation contract](contracts.md#correlation-và-topology).
3. **Có distributed spans thực:** cần trace/span ID, parent hoặc explicit links, tên operation/service, start/end hoặc duration hợp lệ, status và provenance. Chỉ dữ liệu đủ semantics mới thành waterfall; log `trace_id` đơn lẻ không bổ sung các field còn thiếu. Span có lifetime và links được định nghĩa riêng trong [OTel Trace API](https://opentelemetry.io/docs/specs/otel/trace/api/).

Để tăng chất lượng **mức 2**, ứng dụng có thể ghi structured JSON chứa `trace_id`, `span_id`, `trace_flags`; OTel đã quy định tên field cho non-OTLP logs. TailPro vẫn có thể đọc file/stdout qua SSH, không cần OTLP log receiver chỉ để tận dụng các field này. [Non-OTLP log formats](https://opentelemetry.io/docs/specs/otel/compatibility/logging_trace_context/).

Để đạt **mức 3**, app cần instrumentation và propagation thực qua HTTP/gRPC/queue, đồng thời có nơi export/store/query spans. W3C định nghĩa HTTP headers; OTel Propagators API tách inject/extract khỏi carrier và đề cập gRPC metadata. Không nên suy rằng một HTTP header tự đi xuyên mọi thư viện/protocol. [W3C](https://www.w3.org/TR/2021/REC-trace-context-1-20211123/), [Propagators API](https://opentelemetry.io/docs/specs/otel/context/api-propagators/).

Với message, producer phải truyền creation context qua message để consumer liên hệ trực tiếp; batch có thể link nhiều producer spans. Messaging conventions hiện còn trạng thái Development, do đó cần profile/version khi tích hợp, không hardcode một hình dạng span cho mọi SDK. [OTel messaging](https://opentelemetry.io/docs/specs/semconv/messaging/messaging-spans/). UI đề xuất hiển thị `parent`, `span link`, `same correlation ID` là ba loại quan hệ khác nhau; chỉ hiện loại nào có evidence.

## 5. Luồng UX đề xuất: bắt đầu từ log

**Mặc định:** log table → chọn lỗi → Inspector với stack và dòng xung quanh. Từ field ID có hành động “Xem log cùng ID”. Request context hiện ngay cạnh log, không yêu cầu người dùng vào màn hình topology trước. Đây là đề xuất từ pattern điều hướng đã thấy ở Grafana/Datadog, không phải feature hiện đã có.

Khi mở request context, cho thấy scope dạng `production · shop · 4 services · 6/8 active sources`. Tách **phạm vi viewer hiện tại** khỏi **phạm vi tìm request**: có nút mở rộng sang các nguồn đang đọc phù hợp, không tự Discover/Start hoặc vượt grant. Cho biết nguồn nào đang được đọc, stopped/disconnected, không trong scope hoặc chưa có dữ liệu; chỉ nêu tên/metadata nếu đã được cấp quyền. Không dùng “tất cả services” khi chỉ biết một tập nguồn cục bộ. [Scope và grants hiện tại](contracts.md#query-và-mcp-cho-coding-agent).

Kết quả ưu tiên một danh sách theo service/instance có thể thu gọn, nhấn event mở evidence và neighbors. Service/host là chip filter; lỗi và duration explicit là highlight. Event-only giữ marker; nếu sau này có spans, chuyển cùng vùng sang waterfall và có tab logs cho span/trace. Cho phép quay lại log filter ban đầu và giữ selection. Không cần bắt người dùng tự viết query mỗi lần chuyển context.

Hai hành động phải khác nhãn: **“Log cùng trace/span ID”** và **“Log quanh thời điểm này”**. Grafana có cả trace/span filtering và time shifts; thiếu trace ID có thể chỉ còn time-range filter. [Grafana correlation settings](https://grafana.com/docs/grafana/latest/datasources/tempo/configure-tempo-data-source/configure-trace-to-logs/). Suy luận thiết kế: window giúp tìm evidence bị lệch thời điểm, không nâng time proximity thành quan hệ causal.

## 6. Không thấy hoặc thấy thiếu cũng là kết quả có cấu trúc

| Dấu hiệu | Cách trình bày đề xuất | Không được kết luận |
| --- | --- | --- |
| Exact query không có match | “Không có bằng chứng retained trong scope/window này”; cho đổi scope/window. | Request chưa từng chạy hoặc lỗi đã hết. |
| Source stopped, disconnect, eviction, query đạt cap | Coverage theo nguồn, time range thực quan sát, gap reason; load next page hoặc rerun. | Đã thấy toàn bộ request chỉ vì query xử lý xong. |
| Có log trace ID nhưng không có trace/span | “Chưa có span trong dữ liệu hiện có”; ghi sampling/connector status nếu biết. | Instrumentation chắc chắn hỏng hoặc span chắc chắn không tồn tại. |
| Có parent ID nhưng parent không có trong kết quả | Missing parent placeholder với ID, không tự tạo span/duration. | Missing parent chính là root hoặc nguyên nhân lỗi. |
| Hai host có timestamp chồng chéo bất thường | Hiện event clock/observed clock và warning; giữ dữ liệu gốc. | Trừ timestamp để suy network/queue latency. |

Sampling có thể bỏ trace trước khi quan sát toàn trace; tail sampling cần state và tài nguyên để xem nhiều spans trước quyết định. [OTel sampling](https://opentelemetry.io/docs/concepts/sampling/). Collector cũng có thể mất telemetry khi queue đầy, retry hết hạn, crash hoặc storage lỗi. [Collector resiliency](https://opentelemetry.io/docs/collector/resiliency/). Vì vậy TailPro không nên đoán một nguyên nhân duy nhất từ “missing spans”; chỉ nêu nguyên nhân có evidence từ pipeline/source.

Jaeger điều chỉnh clock dựa trên quan hệ causal giữa spans và gắn warning cùng delta; thuật toán vẫn có giả định về request/response latency. [Jaeger clock skew](https://www.jaegertracing.io/docs/2.23/deployment/configuration/#clock-skew-adjustment). TailPro không có đủ semantics này từ event logs, nên không tự căn timestamp cho giống một waterfall hợp lý. Cần giữ riêng `query_complete` trong scope với “end-to-end trace completeness unknown”; field `complete` hiện tại không thể tự chứng minh toàn production đã được quan sát. [ReadResponse và correlation contract](contracts.md).

## 7. Kiến trúc triển khai theo mức nhu cầu

| Bước đề xuất | Phần TailPro cần làm | App/hạ tầng cần có | Điều kiện đi tiếp |
| --- | --- | --- | --- |
| A — SSH logs trước | Service/source identity độc lập repo; scope rõ; ID → retained context; paging/coverage/gaps; reuse query, grants, cancellation, redaction hiện có. | Không bắt buộc sửa app hoặc cài collector. Logs hiện có vẫn dùng được. | Kiểm tra một service nhiều sources và microservice trong cap hiện có; không gọi là distributed tracing. |
| B — Structured correlation | Chuẩn hóa field profiles và provenance; typed request/business/job IDs; service/instance grouping. Bổ sung span ID khi có schema hỗ trợ. | App ghi/propagate IDs qua các boundary thực tế; log JSON khuyến nghị, không bắt buộc đổi transport. | Reproduce qua replica, HTTP/gRPC và queue; kiểm chứng cả propagation bị đứt. |
| C — Connectors tùy chọn | Query adapter cho centralized logs/traces đã có; normalize kết quả vào bounded evidence/span DTO; auth, pagination, retention và provider coverage rõ. Có thể bắt đầu bằng explicit external trace link trước full connector. | Một log/trace backend đang vận hành và quyền truy cập; không yêu cầu mọi user triển khai mới. | Chọn provider cụ thể, kiểm tra API/permissions/limits và không coi search response là full trace. |
| D — Native spans nếu chứng minh cần | Thiết kế ingestion/store/query span riêng có bounds, security, lifecycle và retention; explicit parent/link semantics; query hợp nhất nhưng giữ provenance. | Instrumentation/exporter và luồng OTLP có vận hành/giám sát. | ADR và budget riêng; đây là scope kiến trúc mới, không phải đổi tên màn Trace. |

Đây là thứ tự đề xuất, không tự mở các phase/numbered items. A/B tận dụng modular monolith hiện tại; C/D chưa phải khả năng đã có. Jaeger thực sự nhận OTLP traces và dùng backend lưu spans; đó là khác biệt kiến trúc với TailPro đọc log qua SSH. [Jaeger features](https://www.jaegertracing.io/docs/2.23/features/), [TailPro architecture](architecture.md).

**Scale không đồng nghĩa tăng max8.** Trước khi tăng streams cần đo SSH sessions/processes, ingress bytes, fair scheduling, queue/drop accounting, parsing/query CPU, memory/retention và UI pagination. Source scheduling theo điều tra hoặc truy vấn centralized backend là lựa chọn thiết kế cần benchmark; không tự động mở mọi file của mọi replica. Phải giữ budgets/deadline hiện có hoặc sửa contract có bằng chứng đo. [Architecture budgets](architecture.md), [contracts](contracts.md).

Nền tảng observability đầy đủ là tùy nhu cầu: lưu tập trung, retention và spans hữu ích khi log không còn trên host hoặc số replica vượt khả năng đọc trực tiếp; đổi lại có pipeline, storage, auth và vận hành riêng. Không nên bắt đầu bằng collector/database cho mọi single-service user. Sampling bản thân cũng có chi phí compute/engineering và có thể bỏ thông tin quan trọng. [OTel sampling tradeoffs](https://opentelemetry.io/docs/concepts/sampling/).

## 8. Các tình huống review thủ công trước khi nhận thiết kế

- Single service: access/error/app/worker trên một host; stdout/stderr xen kẽ; không có common ID; lỗi multiline; repo mapping để trống vẫn nhóm service được.
- Microservice: cùng service trên hai host, nhiều replicas, tên service trùng giữa environment/cluster; mở Trace khi viewer chỉ đang lọc một host.
- Correlation: order ID chứa nhiều trace/attempt; một ID nằm ngoài window, nguồn stopped hoặc event đã eviction; shared ID có nhưng thiếu spans.
- Tracing tùy chọn: HTTP/gRPC propagation đứt, queue retry, fan-out/fan-in/batch links, missing parent, clock skew; không biến links thành synchronous child spans.
- Bounds: đạt source/query/page cap; cấp/revoke grant trong khi đọc; connector không trả hết hoặc trace chưa export xong; UI vẫn giải thích scope và partial evidence.

Các tình huống trên là acceptance đề xuất cho công việc tương lai. Nghiên cứu này chỉ đọc code và tài liệu; không chạy live instrumentation, benchmark nguồn hoặc sửa UI/runtime.
