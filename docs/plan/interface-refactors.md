# Kế hoạch giao diện TailPro: tinh gọn, có chủ ý

Ngày 25/11/2026. Trạng thái: đã triển khai phạm vi code B1–B5; nghiệm thu **PARTIAL**. Xem [hồ sơ trước/sau và giới hạn](../process/interface-refactors-acceptance.md). Kế thừa và điều chỉnh từ “TailPro interface refactor plan”.

## 1. Cách tiếp cận

Plan cũ làm tốt việc loại bỏ những thứ không nên có: gradient, glow, panel trong suốt, card lặp lại, badge dày đặc. Bản này giữ nguyên phần đó và bổ sung ba thứ còn thiếu:

1. **Danh tính thị giác bằng quyết định cụ thể**, thay vì chỉ liệt kê những thứ không làm.
2. **Thứ tự làm theo giá trị**: bảng log là nơi người dùng nhìn nhiều nhất nên làm trước, không để đến gần cuối.
3. **Kiểm chứng bằng mắt sớm**, với dữ liệu log thật, thay vì chờ đến bước nghiệm thu.

Nguyên tắc tinh gọn: mỗi phần tử trên màn hình phải trả lời được câu hỏi “nó giúp người dùng chọn nguồn, đọc log, tìm kiếm hay soi sự kiện ở chỗ nào?”. Không trả lời được thì bỏ. Kế hoạch này không thêm connector, toán tử truy vấn, khả năng trace hay thay đổi backend.

Giới hạn cần nói thẳng: không có bộ quy tắc nào bảo đảm giao diện “không giống AI”. Cảm giác đó đến từ các quyết định có chủ ý và độ chính xác khi thực thi. Kế hoạch này ghi các quyết định ra giấy và buộc kiểm tra bằng ảnh chụp thật.

## 2. Năm chữ ký thiết kế

Đây là những chi tiết làm TailPro nhận ra được là TailPro, và đều xuất phát từ chính việc đọc log.

**1. Gutter mức độ.** Một sọc 2px ở mép trái mỗi hàng, màu theo level. Chữ message giữ màu trung tính, không tô nền cả hàng. Level vẫn có nhãn chữ để màu chỉ là tín hiệu phụ.

**2. Timestamp có phân cấp.** Ngày hiển thị mờ, giờ:phút:giây rõ nhất, phần mili giây mờ. Dùng số tabular để các hàng thẳng cột. Định dạng theo kiểu thực tế của nguồn, không ép về một chuẩn.

**3. Tô nhẹ cấu trúc trong message** (tùy chọn, làm cuối bước 2). Chỉ tô tối đa ba loại token như key=value, khoá JSON, ID/UUID, đường dẫn, bằng các mức sáng khác nhau của màu trung tính cộng một sắc nhấn. Không tô nhiều màu. Phải có công tắc tắt, và nội dung sao chép ra phải giữ nguyên văn bản gốc.

**4. Đánh dấu kết quả tìm kiếm.** Đánh dấu đúng đoạn khớp trong message bằng nền nhấn nhạt kèm gạch chân, để người đọc thấy ngay vì sao hàng này xuất hiện.

**5. Thanh trạng thái như dụng cụ đo.** Một dòng 24px, chữ Mono 12px: trạng thái kết nối, số nguồn đang đọc, phạm vi query hiện tại, trạng thái tạm dừng. Không icon trang trí, không card.

**Quy tắc dùng màu nhấn.** Màu nhấn chỉ xuất hiện ở ba nơi: nút chính, chỉ báo đang chọn, đoạn khớp tìm kiếm. Mọi chỗ khác dùng độ sáng của màu trung tính. Nhấn ít thì khi nó xuất hiện mới có nghĩa.

## 3. Nền tảng thị giác

Phần chi tiết token, thang chữ, hình học đã có trong plan cũ (mục 3). Bản này chỉ chốt thêm các điểm sau.

**Màu.** Giữ bộ token màu của plan cũ làm baseline vì đã có tính tương phản. Mọi thay đổi hex phải tính lại tỉ lệ tương phản trước khi dùng. Mục tiêu giữ nguyên: chữ thường ≥ 4.5:1, dấu hiệu điều khiển và trạng thái ≥ 3:1.

**Chữ.** IBM Plex Sans cho giao diện và IBM Plex Mono cho log, đóng gói WOFF2 cục bộ cùng giấy phép, không gọi CDN. Chốt Plex làm mặc định. Chỉ đổi khi bảng mẫu chữ (specimen) lộ lỗi thật: dấu tiếng Việt, phân biệt 0/O và 1/l/I, tắt ligature trong log.

**Cỡ chữ tối thiểu.** Không có chữ nào người dùng cần đọc dưới 12px. Cỡ 11px của plan cũ chỉ còn dành cho nhãn phụ rất hiếm, và nên bỏ hẳn nếu không cần. Thang cốt lõi: 18, 14, 13, 12.

**Hình học.** Thang khoảng cách 4, 8, 12, 16, 24, 32px. Bo góc 4/6/10px. Viền 1px. Đổ bóng chỉ cho menu và dialog. Kiểm tra viền 1px còn sắc nét ở hệ số hiển thị 125% và 150%, vì đây là nơi giao diện desktop dễ trông “nhòe”.

**Chuyển động.** 100–140ms cho đổi màu hoặc độ mờ, không hiệu ứng vào màn hình, tôn trọng reduced motion.

## 4. Bố cục

Giữ khung của plan cũ: sidebar 240px, header 48px, hàng tab nguồn 36px, toolbar một dòng, bảng log chiếm phần lớn, Inspector 400px chỉ mở khi cần, thanh trạng thái 24px. Quy tắc cho cửa sổ hẹp (≥1280, 1000–1279, 800–999, dưới 800) giữ nguyên như plan cũ mục 4.1.

Bổ sung các điểm sau:

- Không bọc toàn bộ vùng xem log trong card. Các vùng gặp nhau bằng đường kẻ thẳng, không lồng khung bo tròn.
- Sidebar dành chiều cao cho danh sách nguồn. Connections và Settings đặt ở khu tiện ích dưới cùng, vẫn mở cửa sổ riêng như hiện tại.
- Hàng tab nguồn: tên log ngắn, ngữ cảnh host gọn, đường dẫn đầy đủ nằm trong tooltip. Tab đang chọn dùng gạch chân, không dùng khối màu lớn. Nút đóng tab phải ghi rõ trong tên truy cập là “dừng đọc log”.
- Thông báo phạm vi và retention gộp thành một dòng tóm tắt có thể mở rộng, không bỏ cảnh báo hay hành động khôi phục nào.
- Không thêm thẻ thống kê phía trên bảng log.

Mục tiêu chiều cao vùng log ở trạng thái mặc định giữ như plan cũ (≥70% ở 1440×900, ≥65% ở 1366×768) và chỉ tính là đạt sau khi đo trên bản render thật.

## 5. Các thành phần điều khiển

Chi tiết kích thước và trạng thái đã có ở plan cũ mục 5. Bản này chốt các nguyên tắc để toàn bộ app trông như một hệ thống.

| Thành phần | Quyết định |
| --- | --- |
| Nút | Mỗi vùng chỉ có tối đa một nút primary. Toolbar dùng ghost. Cao 32px, nút trong form 36px. Có đủ trạng thái mặc định, hover, nhấn, focus-visible, vô hiệu, đang tải với cùng hình học. |
| Ô nhập | Nhãn luôn hiển thị, placeholder chỉ đưa ví dụ. Khoảng cách thuộc về container của field, không đặt margin toàn cục lên input và label. |
| Select | Dùng wrapper bọc select gốc cho danh sách ngắn cố định. Không thay bằng popup tự viết chỉ để đổi giao diện. |
| Menu hành động | Hàng 32px, icon căn trái, phím tắt căn phải, tự đặt lại vị trí trong viewport, đóng khi bấm ngoài hoặc Escape. |
| Tab | Gạch chân cho nội dung, segmented control chỉ cho chế độ xem loại trừ nhau. |
| Tooltip | Giải thích icon và hiện phần bị cắt. Không chứa cảnh báo thiết yếu. Hiện khi focus bàn phím. |
| Icon | Một bộ SVG, nét thống nhất, 16px trong điều khiển. Chỉ dùng icon khi chữ không đủ rõ. |

Vòng focus phải đạt ít nhất 3:1 so với nền liền kề và nhìn thấy được trên cả nền sidebar, bảng và menu.

## 6. Bảng log và Inspector

**Bảng log.**

- Hàng 32px, header 28px, chữ Mono 13px, một dòng mỗi hàng, giữ virtualization.
- Cột ưu tiên: Time, Level, Message. Hiện thêm Source khi xem nhiều nguồn. Giữ thứ tự và lựa chọn cột của người dùng.
- Nền hàng trung tính, đường phân cách mảnh, không kẻ sọc đậm, không tô nền cả hàng theo mức độ.
- Phân biệt hàng đang focus bằng bàn phím với sự kiện đang mở ở Inspector. Việc chọn không chỉ dựa vào màu xanh.
- Giữ nguyên hành vi hiện có: click để chọn, nhấp đúp hoặc Enter để mở chi tiết, Pause/Resume, tải trang cũ hơn, neo vị trí, phạm vi nguồn hiện tại.
- Số đo chẩn đoán chuyển ra khỏi luồng đọc. Mất coverage, hết hạn, bị cắt và hành động khôi phục vẫn hiển thị khi liên quan.

**Cảnh báo kỹ thuật bắt buộc.** Đổi chiều cao hàng phải sửa đồng thời CSS, vị trí hàng, tổng chiều cao ảo, tính vùng nhìn thấy, cuộn đến hàng được chọn và tính neo khi tạm dừng. Dùng một nguồn giá trị duy nhất cho chiều cao hàng, không sửa riêng `.log-row`.

**Inspector.**

- Rộng 400px khi màn hình rộng, có thể kéo, đổi cỡ bằng bàn phím và đặt lại.
- Ba tab chính: Fields, Raw, Stack. Các mục nâng cao nằm trong menu More, và khi một mục nâng cao đang mở thì tên nó hiện ở khu tab để người dùng luôn có đường quay lại.
- Fields hiển thị dạng key/value căn thẳng. Thay hai nút copy lặp lại ở mỗi dòng bằng hành động theo dòng, hiện khi hover hoặc focus. Giữ phân biệt copy trường và copy giá trị cùng các quy tắc cắt bớt dữ liệu.
- Export và Explain Error là hành động phụ. Giữ nguyên cơ chế opt-in, đồng ý và coi kết quả AI là giả thuyết chưa kiểm chứng.
- Đóng Inspector trả focus về vùng log và giữ vị trí đọc.

## 7. Các bề mặt quản lý

- **Connections:** danh sách phẳng, host và metadata bên trái, môi trường và hành động bên phải. Phân biệt hồ sơ đã lưu với kết nối đang chạy bằng tiêu đề mục, không dùng card nặng. Một hành động chính “Add connection”.
- **Trình sửa kết nối:** field xếp dọc, User và Port có thể chung một hàng. Test connection ở bên trái, Cancel và Save bên phải. Giữ các phân biệt sẵn có: lưu hồ sơ không kết nối, test thành công không tự lưu. Giữ đồng ý SSH động, kiểm tra host key và nhãn Production.
- **Sources và parser:** danh sách chọn nguồn rõ ràng, mỗi hàng một hành động. Tách khám phá, nhập đường dẫn thủ công và cấu hình connector. Lỗi quyền chặn không được giấu trong mục nâng cao.
- **Settings, Trace, Topology, dialog:** dùng cùng kiểu chữ, kích thước điều khiển và đường kẻ. Không thiết kế lại mô hình sản phẩm của Trace và Topology. Dialog giữ `WorkspaceDialog` hiện có với chiều rộng 440/560/720px tùy loại.

**Điều chỉnh phạm vi25/11/2026 theo yêu cầu:** gỡ menu ⋯ và Find host or log/Ctrl+K, Sources, Trace, Topology khỏi desktop UI; bỏ Discover sources và panel công cụ. Cây HOSTS vẫn thêm absolute file path và Start bằng double-click/Enter. Connections/+ mở thẳng form SSH có Connect, Test connection tùy chọn và Cancel; không còn bước danh sách → Add → Save. Các mục nghiệm thu B5 cho panel đã bỏ chỉ là evidence lịch sử.

Các trạng thái rỗng, đang tải, lỗi, ngoại tuyến, tạm dừng và dữ liệu một phần giữ đúng bảng yêu cầu của plan cũ mục 8. Không biến dữ liệu thiếu thành kết quả rỗng thông thường.

## 8. Danh sách kiểm “không giống AI”

Dùng danh sách này sau bước 2 và lặp lại ở bước nghiệm thu. Đây là công cụ phát hiện sớm, không phải bảo chứng.

| Hạng mục | Câu hỏi kiểm tra | Đạt khi |
| --- | --- | --- |
| Màu nhấn | Màu nhấn xuất hiện ở những loại phần tử nào? | Chỉ nút chính, chỉ báo chọn, đoạn khớp tìm kiếm |
| Hình khối | Có card trong card, khung bo lồng nhau, pill thừa không? | Không có |
| Hiệu ứng | Có gradient, glow, nền mờ, bóng ngoài menu và dialog không? | Không có |
| Icon | Icon có thay được bằng chữ rõ hơn không? | Chỉ giữ icon nơi chữ không đủ, cùng một nét |
| Câu chữ | Nhãn có cụ thể không? | Dùng “Dừng đọc log” thay vì “Quản lý”, không emoji, không câu quảng cáo |
| Khoảng cách | Mọi khoảng trống có thuộc thang 4/8/12/16/24/32 không? | Không có giá trị lẻ ngoài thang, trừ hình học riêng có chủ đích |
| Dữ liệu mẫu | Ảnh chụp dùng dữ liệu nào? | Log thật đã ẩn danh, host và đường dẫn dài, stack nhiều dòng, tiếng Việt. Không dùng “Item 1” |
| Trạng thái | Rỗng, lỗi, đang tải có thiết kế riêng không? | Có, không để mặc định của trình duyệt |
| Nhận diện | Che logo đi thì còn nhận ra là TailPro không? | Còn ít nhất một chi tiết nhận ra, ví dụ gutter mức độ và timestamp phân cấp |
| Ý kiến thứ hai | Có người không tham gia thiết kế xem thử chưa? | Đã xem trên ảnh chụp thật và ghi lại nhận xét |

Có thể chạy thêm các công cụ bên thứ ba như Impeccable (`/critique`) hoặc `detect.mjs` của avoid-ai-design như một lượt kiểm tra độc lập. Coi kết quả là gợi ý, không phải thước đo quyết định. Đọc mã của công cụ trước khi cài vào dự án.

## 9. Lộ trình năm bước

So với plan cũ có bảy hạng mục, lộ trình này gộp còn năm và đưa bảng log lên sớm. Quy tắc di chuyển CSS ở plan cũ mục 9 vẫn giữ: mỗi điều khiển đổi markup và CSS cùng lúc, xoá khai báo cũ ngay trong bước đó, không thêm selector toàn cục kiểu `.primary` mới, không trộn với tái cấu trúc backend, không đụng vào thay đổi chưa commit của người khác.

| Bước | Phạm vi | Hoàn thành khi |
| --- | --- | --- |
| **B1. Nền móng** (tương ứng UI-01) | Chụp ảnh baseline ở 1440×900, 1366×768, 1024×768, 800×600. Chuẩn bị bộ log mẫu ẩn danh. Đóng gói font cục bộ. Tạo `tokens.css`, `base.css`. Làm bảng mẫu chữ có tiếng Việt. | Font chạy khi ngoại tuyến. Bảng mẫu token và chữ xem được. Chưa tuyên bố bề mặt nào đã chuyển. |
| **B2. Bảng log và chữ ký** (đưa UI-04 lên sớm) | Chỉ vùng bảng: hình học hàng, gutter mức độ, timestamp phân cấp, đánh dấu tìm kiếm, trạng thái chọn. Tô cấu trúc message làm cuối và đo hiệu năng trước khi giữ. | Ảnh chụp thật qua danh sách kiểm mục 8. Cuộn, tạm dừng, neo, tải trang cũ, cột, phạm vi nguồn kiểm tra tay. **Cổng dừng:** nếu chưa có chữ ký nào thấy ổn, quay lại sửa trước khi làm tiếp. |
| **B3. Điều khiển dùng chung và khung** (UI-02 + UI-03) | Button, TextField, Select, Icon. Áp vào QueryBar và trình sửa kết nối trước. Sau đó sidebar, tab nguồn, toolbar, thanh trạng thái. | Mọi trạng thái đã duyệt. Tên dài vừa khít. Tiện ích vẫn với tới được. Chuyển tab không dừng reader, đóng tab vẫn chỉ dừng đúng reader. |
| **B4. Inspector và bề mặt nổi** (UI-05) | Inspector, thanh kéo, `WorkspaceDialog`, menu ứng dụng, command palette. | Kéo, đóng, focus, sao chép, đường về mục Evidence và Original bytes còn nguyên. |
| **B5. Còn lại và nghiệm thu** (UI-06 + UI-07) | Connections, Sources, ParserSettings, Settings, export, ExplainError, Trace, Topology. Dọn CSS cũ. Ghi hồ sơ nghiệm thu. | Ma trận kích thước, trạng thái, bàn phím đã duyệt. Build qua. Kiểm tra chưa chạy được thì ghi rõ. |

Rủi ro của việc làm bảng log trước điều khiển dùng chung là phải chỉnh lại toolbar sau. Giảm bằng cách giới hạn B2 trong vùng bảng, không đụng toolbar và QueryBar cho tới B3.

Không ước lượng thời gian lịch cho tới khi B1 cho thấy số trạng thái cần phủ và phần điều khiển nào cần sửa hành vi.

## 10. Nghiệm thu

Giữ ma trận nghiệm thu của plan cũ mục 11: bốn kích thước cửa sổ với Inspector mở và đóng, luồng đầy đủ từ mở nguồn đến tìm kiếm, tạm dừng, tải cũ hơn, chọn, soi, sao chép, đóng, tiếp tục bằng chuột và bàn phím, kiểm tra tương phản trên bản render cuối, kích thước vùng bấm, tên truy cập, reduced motion và zoom 200%.

Quy tắc kiểm chứng của repository vẫn áp dụng: **không thêm test tự động hay hạ tầng test cho việc này**. Chạy `npm run build` (đã gồm typecheck) sau mỗi lát cắt frontend đáng kể và nghiệm thu bằng tay.

Bổ sung cho mục tiêu thị giác:

- Danh sách kiểm mục 8 đạt trên ảnh chụp thật, không chỉ trên bản dựng sạch.
- Có ảnh trước/sau và hồ sơ duyệt ngắn lưu trong tài liệu quy trình hiện có.
- So sánh hiệu năng cuộn và cập nhật nguồn với baseline cùng khối lượng dữ liệu, cùng máy, cùng cấu hình build. Ghi nhận thời gian khung hình và bộ nhớ. Không kết luận đạt hiệu năng chỉ từ mẫu khi nhàn rỗi.
- Build thành công không chứng minh chất lượng thị giác, khả năng truy cập, tính đúng khi chạy hay hiệu năng.

## 11. Giả định và rủi ro

- Kế hoạch dựa trên tài liệu, chưa dựa trên ảnh chụp của ứng dụng đang chạy. Mọi nhận định thị giác cần xác nhận trên cửa sổ Tauri thật ở B1.
- Tô cấu trúc message có thể tốn hiệu năng và gây lệch nội dung sao chép. Chỉ tính cho hàng đang hiển thị, đo trước khi giữ, có công tắc tắt.
- Bộ màu charcoal với một sắc nhấn dịu và cặp font Plex là lựa chọn phổ biến. Danh tính của TailPro vì thế phải đến từ năm chữ ký ở mục 2 và độ chính xác khi thực thi, không đến từ riêng bảng màu hay font.
- Không thể hứa là giao diện sẽ không bị cảm thấy giống AI. Có thể cam kết là các quyết định đều có chủ ý, được ghi lại và được kiểm tra bằng dữ liệu thật.

## 12. Việc đầu tiên

Làm B1, rồi sang B2 ngay: chụp baseline và bộ log mẫu, đóng gói font, tạo token, sau đó áp gutter mức độ, timestamp phân cấp và trạng thái chọn lên bảng log. Xem kết quả trên ảnh chụp thật trước khi mở rộng sang phần còn lại của ứng dụng.


## 13. Kết quả triển khai — 25/11/2026

Đã áp font/token và primitives chung; bảng32px/header28px dùng geometry chung, gutter mức độ, timestamp phân cấp, highlight literal/message và status24px. Inspector400px có ba tab chính/More, row copy actions, resize/reset/focus; các cửa sổ quản lý và công cụ còn lại dùng cùng hệ thống. Connections/Settings ở cuối sidebar theo mục4; minimum desktop800×600. Không sửa backend hay thêm dependencies/tests.

B1–B5 đã có code và browser smoke; build frontend và desktop bundled-assets PASS. Specimen HTML là artifact lịch sử hiện không còn trong repo; kết quả và giới hạn nằm ở [hồ sơ nghiệm thu](../process/interface-refactors-acceptance.md). Gate hình ảnh sơ bộ B2 đã xem được gutter/timestamp trên render, nhưng chưa thay nghiệm thu log thật/native. Tô cấu trúc message tùy chọn deferred; chưa có log thật đã ẩn danh, reviewer độc lập hoặc sustained native/RSS measurement. Không đánh dấu toàn bộ nghiệm thu mục10 đạt.
