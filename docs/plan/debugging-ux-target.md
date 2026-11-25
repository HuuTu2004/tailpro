# Thiết kế trải nghiệm debug: đơn service và microservice

Ngày 25/11/2026. Đây là thiết kế mục tiêu để review, chưa phải xác nhận tính năng đã triển khai. Cơ sở là [nghiên cứu nguồn chính thức](../system-design/microservice-debugging-ux-research.md). Các giới hạn code hiện tại không quyết định hình dạng sản phẩm.

## 1. Mục tiêu và công việc chính

Một công cụ để tìm hiểu chuyện gì xảy ra trong hệ thống đang vận hành: mở log nhanh, tìm lỗi, xem ngữ cảnh, lần theo request khi có bằng chứng và tới đúng service/instance cần kiểm tra. Cùng trải nghiệm cho app đơn, nhiều process/worker, microservices, containers, Kubernetes và nguồn telemetry tập trung/cloud. Hỗ trợ từng connector phải có ma trận khả năng kiểm chứng riêng; không tuyên bố mọi protocol/provider đã chạy.

Người dùng chỉ cần trả lời: đang xem hệ thống nào, môi trường nào và khoảng thời gian nào. Không phải hiểu parser, epoch timestamps, nguồn clock, source IDs hoặc span semantics mới đọc được log.

## 2. Một vùng làm việc chính

```text
Shop platform ▾     Production ▾      Last 15 minutes ▾
Search message / request ID / order ID…

Logs   [Request · 4bf92f… ×]                  6/7 sources
┌──────────────────────────────────────────────────────┐
│ Danh sách log hoặc hành trình request đang xem       │
│                                                     │
│ Chi tiết chỉ mở khi chọn; kéo chia độ rộng hai phần  │
└──────────────────────────────────────────────────────┘
```

- Mặc định vào Logs, bảng chiếm toàn vùng. Có một thanh chọn service/nguồn gọn; app chỉ có một service thì tự chọn, không bắt tạo project hoặc nhập repo.
- Request là tab theo ngữ cảnh: bấm ID/hành động từ log mới mở, giữ bộ lọc Logs khi quay lại, có thể đóng tab. Không thêm một màn dashboard bắt người dùng đi qua trước.
- Chọn event mở chi tiết ngay trong khung chung, xem raw/fields/stack hoặc context. Chi tiết đóng thì bảng trở lại full width. Giữ khả năng kéo divider và scrollbar ngang nhìn thấy được.
- Connections và Settings là các cửa sổ quản trị riêng. Chỉ mở khi cần cấu hình; mỗi nơi có một nhiệm vụ. Kết nối được nhưng chưa có quyền/nguồn log phải giải thích đúng trạng thái.
- Cảnh báo chỉ hiện khi tác động việc điều tra: nguồn chưa đọc, dữ liệu hết retention, query chưa tải hết, ID không liên kết được. Chi tiết kỹ thuật nằm trong phần mở rộng.

## 3. Luồng sử dụng thực tế

| Trường hợp | Luồng thao tác | Kết quả cần thấy |
| --- | --- | --- |
| App đơn, nhiều file | Kết nối → chọn app.log/error.log/access.log → All hoặc chọn vài nguồn → tìm lỗi | Một bảng, stack không bị tách vô nghĩa, host/file chỉ là thông tin bổ sung |
| App đơn nhiều replica/worker | Chọn service → mặc định gộp instance → bấm instance phát lỗi khi cần | So sánh được các replica, không phải mở lần lượt từng host |
| Microservice có ID chung | Chọn lỗi → Xem request → tìm trên nhóm service đúng môi trường | Log cùng ID theo service/instance; không bị bộ lọc một file của Logs âm thầm giới hạn |
| Có distributed spans | Mở request → chọn service/operation trong timeline → log tương ứng | Có thời lượng/quan hệ từ span thực, giữ log context, chỉ mở inspector khi chọn |
| Queue/worker/retry | Từ request mở tác vụ/trace liên quan → chọn attempt/consumer | Async link và retry tách rõ; nhiều message/trace không bị nhập thành một request giả |
| Legacy microservice chưa có ID chung | Chọn service, khoảng thời gian và nội dung → đối chiếu log/context | Dùng được ngay; không vẽ hành trình request hay kết luận root cause chỉ vì hai dòng gần giờ |
| Thiếu log | Bấm nhãn coverage → thấy nguồn/instance nào chưa đọc hoặc hết dữ liệu → Thêm nguồn/mở rộng thời gian | Thiếu bằng chứng là trạng thái rõ ràng, không giống “không có lỗi” |

Kiểu triển khai và mức telemetry là hai chiều độc lập: single service vẫn có thể có spans HTTP/DB; microservice vẫn có thể chỉ có text logs. UI dựa vào dữ liệu thực có, không khóa tính năng theo tên “monolith/microservice”.

## 4. Request context dễ dùng

Phạm vi tìm request thuộc hệ thống/môi trường đang điều tra, được hiển thị rõ và tách khỏi bộ lọc nguồn tạm thời của bảng Logs. Người dùng có thể thu hẹp service sau khi đã thấy phạm vi. Không tự kết nối hoặc đọc một nguồn chưa được cho phép để làm kết quả trông đầy đủ.

Hành trình ưu tiên các hàng theo service/operation, thu gọn replica mặc định. Bấm hàng lọc log bên dưới; bấm event mở chi tiết. Error là highlight, không tự giấu hết các log Info giải thích sự cố. Với nhiều kết quả theo order/job ID, hiện danh sách request/attempt để chọn, không gom tất cả vào một trace.

Có span thực thì dùng timeline có thời lượng và parent/link rõ. Chỉ có shared ID thì hiển thị các event tương quan. Không có ID thì giữ đối chiếu log theo điều kiện tìm. “Log cùng ID” và “Log quanh thời điểm này” là hai hành động khác nhau.

Thông tin tóm tắt vừa đủ: operation, ID, thời điểm, số service có evidence, lỗi quan sát và duration nếu có. Không gọi số lỗi là error rate; không tự chỉ định nguyên nhân gốc hoặc đường găng khi thiếu dữ liệu.

## 5. Giữ, đưa vào phần phụ và loại khỏi mặc định

| Quyết định | Tính năng | Lý do |
| --- | --- | --- |
| Giữ ở luồng chính | Search, chọn service/nguồn, time range dễ đọc, pause/live, context, request từ ID, details theo ngữ cảnh | Trực tiếp giúp điều tra một sự cố |
| Chỉ hiện khi cần | Replica/host/file, raw/fields/stack, source coverage, parser mapping | Chi tiết quan trọng nhưng không cần đồng thời chiếm màn hình |
| Cấu hình riêng | Connections, quyền đọc, Settings, quy tắc field khi auto-detect chưa đủ | Làm lúc thiết lập hoặc thay đổi dữ liệu, không chen vào mỗi lần đọc log |
| Không bắt buộc | Repo/code mapping, instrumentation/collector, cloud account, tạo project | Người dùng phải đọc được logs hiện có trước |
| Không đưa vào trang mặc định | Service topology toàn hệ thống, incident inbox cố định, nhiều kiểu graph/waterfall cùng lúc | Tăng số panel và quyết định trước khi giúp tìm được evidence |
| Bỏ cách nhập khó dùng | Epoch milliseconds, source IDs, form Compose/repo như cửa ngõ để debug | Dùng time picker, tên service, nguồn đọc được; kỹ thuật nâng cao là phần phụ |
| Không tạo | Tự nối request từ timestamp, tự coi business ID là trace ID, dự đoán duration/root cause khi không có bằng chứng | Đưa ra kết quả dễ hiểu nhưng sai sẽ làm điều tra khó hơn |

Các layout Incident desk và Service flow chỉ là bản so sánh trong prototype. Đề xuất chọn Request timeline làm hướng chính; graph chỉ cân nhắc như góc nhìn phụ khi có spans đủ quan hệ và người dùng có nhu cầu cụ thể.

## 6. Phạm vi hệ thống hiện đại

| Môi trường | Đường lấy dữ liệu mục tiêu | Trải nghiệm thống nhất |
| --- | --- | --- |
| Local/VM/VPS | File, stdout/stderr đã được đưa tới nguồn đọc được, Journal, SSH | Chọn nguồn → đọc/gộp log → context |
| Docker/Compose | Container log streams và metadata service/instance | Chọn service trước, container khi cần |
| Kubernetes | Context/namespace/workload/pod/container, restart/previous-instance metadata khi nguồn hỗ trợ | Gộp workload/replica; thiếu quyền/log đã rotate phải hiện rõ |
| Centralized logs hoặc cloud/serverless | Connector API của hệ thống đã có, như Loki/Elasticsearch/CloudWatch; chọn provider và quyền cụ thể | Cùng search/context/request, giữ paging và phạm vi dữ liệu provider |
| Distributed tracing | Connector backend traces hoặc span input được xác định riêng; correlation theo tiêu chuẩn OTel/W3C | Từ log sang trace và từ operation/span về log |

Đây là ma trận thiết kế mục tiêu, không phải danh sách connector đã hoàn thành. Không dùng SSH như giả định chung cho Kubernetes/serverless; không bắt user dựng cả nền tảng observability chỉ để đọc một app. Logging cluster thường có vòng đời/retention khác Pod; cần biết giới hạn của nguồn. [Kubernetes logging](https://kubernetes.io/docs/concepts/cluster-administration/logging/). Cloud API có pagination riêng, thậm chí trang rỗng chưa có nghĩa đã hết dữ liệu. [CloudWatch GetLogEvents](https://docs.aws.amazon.com/AmazonCloudWatchLogs/latest/APIReference/API_GetLogEvents.html).

## 7. Tiêu chí đánh giá thiết kế

Người dùng bắt đầu từ log lỗi phải tìm được context, nhận diện service/instance và mở request cùng ID bằng các hành động trực tiếp, không copy ID qua nhiều cửa sổ. Khi không có ID, dùng log explorer bình thường. Chuyển scope không làm mất trạng thái đọc cũ; thiếu data không bị mô tả là kết quả đầy đủ. App đơn không bị thêm việc cấu hình chỉ để thỏa mô hình microservice.

Review tối thiểu: app đơn nhiều file; một service nhiều replica; request qua HTTP/gRPC; queue retry/fan-in; order ID có nhiều requests; microservice không common ID; staging/production trùng tên; Pod restart; clock skew; source offline; retention/query cap. Đây là acceptance đề xuất; nghiên cứu và prototype không thay cho chạy các hệ thống thật.

## 8. Prototype tham khảo

Các layout A/B/C là nghiên cứu giao diện trên branch riêng `prototype/microservice-debugging-ux`, không nằm trong runtime chính. Source tree chính đã gỡ demo IPC/fixtures trong đợt cleanup25/11/2026. Các thiết kế trong tài liệu này chưa được promote thành tính năng production; chạy hệ thống thật bằng desktop theo README.
