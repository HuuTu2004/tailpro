# Interface refactors — implementation và nghiệm thu

Ngày 25/11/2026, Asia/Bangkok. Theo [plan B1–B5](../plan/interface-refactors.md). **Code đã triển khai; nghiệm thu PARTIAL.** Không coi browser smoke hoặc build là chứng nhận native/log thật/performance.

## Phạm vi bàn giao

| Bước | Đã triển khai | Giới hạn nghiệm thu |
| --- | --- | --- |
| B1 | Plex Sans regular/semibold + Mono regular complete WOFF2, OFL, token/base, specimen tiếng Việt; baseline browser bốn kích thước và native 1280×840 | Chưa có bộ log thực tế đã ẩn danh để duyệt toàn bộ matrix |
| B2 | Hàng32px/header28px; một nguồn geometry cho CSS/offsets/anchors/paging/keyboard; gutter2px, timestamp phân cấp, highlight literal/message, focus và Inspector selection khác nhau | Cổng hình ảnh sơ bộ đã xem; nghiệm thu loaded native và log thật chưa đạt. Tô cấu trúc message tùy chọn chưa bật vì chưa đo được lợi ích/chi phí đầy đủ |
| B3 | Button/TextField/Select/Icon dùng chung; QueryBar và editor; sidebar240px, tab36px gạch chân, Connections/Settings cuối sidebar, status24px Mono12px | Đã kiểm tra luồng qua adapter ngoài repo; SSH/reader thật chưa xác nhận trong đợt UI này |
| B4 | Inspector400px, resize bằng pointer/keyboard/reset; Fields/Raw/Stack, More giữ đường vào Evidence/Original bytes; copy theo menu từng dòng; WorkspaceDialog440/560/720 và menu giới hạn viewport | Clipboard đã kiểm tra đối số bằng adapter trình duyệt; clipboard hệ điều hành, deep/giant/retention matrix đầy đủ còn thiếu |
| B5 | Connections dạng hàng phẳng; Sources/parser/Settings/Trace/Topology/export/Explain dùng cùng token/control; bỏ CSS cũ và selector primary toàn cục | Chưa có người duyệt độc lập; target accessibility, native scaling/zoom và sustained load còn thiếu |

Backend Rust, IPC DTO, quyền, redaction, budgets, parser/query engine không đổi. Tauri chỉ đổi minimum width900→800 để cho phép kích thước nghiệm thu800×600. Không thêm package, automated tests hoặc test infrastructure. Không đưa adapter/demo/fixtures vào runtime/repo.

Geometry Inspector lưu chiều rộng người dùng muốn riêng với giới hạn cửa sổ: thu nhỏ rồi mở rộng không làm mất mặc định400px. Cột đang chọn giữ thứ tự các cột còn lại khi bật/tắt; Source mặc định chỉ hiện khi scope có nhiều nguồn. Literal decoration dùng query của page đã nhận, phân biệt hoa/thường như engine; không highlight giá trị của predicate ID/field như thể chúng khớp message. Chỉ decorate hàng ảo đang render, cap128 phần tử/row; textContent giữ nguyên. Hit từ original bytes/fragment không có trong message preview không được suy đoán thành highlight.

## Ảnh trước/sau

Các ảnh có log dưới đây là **synthetic IPC render review**, không phải log SSH thật. Adapter tạm chỉ nằm ở `/tmp`, inject bằng DevTools; app vẫn dùng native IPC. Native screenshot riêng dùng bundle thật với config/runtime directory tạm, không host/source, không kết nối server. Ảnh không chứa dữ liệu khách hàng, key hay config thật.

| Cửa sổ | Trước | Sau | Inspector mở |
| --- | --- | --- | --- |
| 1440×900 | [Before](interface-review/before-1440.png) | [After](interface-review/after-1440.png) | [Inspector](interface-review/inspector-1440.png) |
| 1366×768 | [Before](interface-review/before-1366.png) | [After](interface-review/after-1366.png) | [Inspector](interface-review/inspector-1366.png) |
| 1024×768 | [Before](interface-review/before-1024.png) | [After](interface-review/after-1024.png) | [Inspector](interface-review/inspector-1024.png) |
| 800×600 | [Before](interface-review/before-800.png) | [After](interface-review/after-800.png) | [Inspector](interface-review/inspector-800.png) |

Native1280×840: [baseline](interface-review/native-before-1280.png), [bundle sau refactor](interface-review/native-final.png). Capture native sau resize lớn có vùng đen/stale backing; không dùng các capture đó để đánh dấu native resize matrix đạt. X11 WM_NORMAL_HINTS của bundle xác nhận minimum800×600. Browser resize matrix chỉ xác nhận layout frontend.

Các bề mặt bổ sung: [editor600×560](interface-review/editor.png), [Connections760×600](interface-review/connections.png), [Settings](interface-review/settings.png), [tên dài/tiếng Việt/RFC3339Nano](interface-review/long-names.png), [specimen](interface-review/specimen.png). [Specimen HTML](interface-specimen.html) mở qua Vite tại `/docs/process/interface-specimen.html` hoặc trực tiếp ngoại tuyến; cả ba font faces đã load, resource origins chỉ có localhost.

Ảnh [125%](interface-review/scale-1.25.png), [150%](interface-review/scale-1.5.png) dùng browser deviceScaleFactor. [200% equivalent](interface-review/zoom-200.png) dùng720×450 CSS pixels ở DPR2, tương ứng không gian1440×900 tại zoom200%; không thay thế thao tác zoom trong Tauri thật.

## Kết quả kiểm tra UI

Browser Chrome headless, Vite frontend trên cùng máy. Đây là một lượt thao tác nghiệm thu qua DevTools, không phải bộ test/harness được thêm vào repository.

| Kiểm tra | Quan sát |
| --- | --- |
| Geometry mặc định | Bounding box row32px, header28px, footer24px; grid/virtual offsets32px; không page overflow ngang ở bốn kích thước khi Inspector đóng/mở |
| Diện tích đọc | Viewport gồm sticky header:679/900=75,4% tại1440;547/768=71,2% tại1366. Loại header28px vẫn đạt72,3% và67,6%, vượt mục tiêu70%/65% trong browser |
| Chữ ký | Gutter2px với nhãn level, message trung tính; timestamp giữ nguyên ngày/zone/nanosecond spelling; highlight nền + gạch chân; status hiển thị connected/readers/scope/query/paused |
| Query highlight | `message:GET` tạo các mark có text `GET`; `trace_id:req-42` không tạo mark message. Thay query chờ page mới trước khi đổi decoration |
| Pause/navigation | Space pause, arrows chọn, Enter mở details; older-page gửi cursor/anchor, cache còn bounded; anchor expired hiện lỗi + Reload retained logs; reload hết lỗi và có rows trở lại |
| Inspector | Mở rộng400px; ArrowLeft410px; double-click reset400px; pointer kéo thêm72px đạt472px; sau resize nhỏ/lớn trở lại400px nếu chưa đổi preference |
| Focus | More+Escape chỉ đóng menu; đóng Inspector trả focus viewport. Pointer mở Export rồi Escape trả focus đúng trigger Export |
| Copy | Menu từng field gọi clipboard với tên trường hoặc giá trị decoded tương ứng; truncated/container vẫn giữ điều kiện vô hiệu. Đây là clipboard argument review, chưa clipboard OS |
| Menu | Bấm pointer ngoài đóng menu; Escape đóng và trả focus. Menu cuối sidebar800×600 nằm trong viewport (top415,bottom553,left8,right210) |
| Reader lifecycle | Combine gửi hai ID đang mở; quay về tab đơn không Stop; × gửi đúng một Stop cho source đó; Settings/Connections gửi đúng section riêng |
| Connection editor | Test chỉ gọi `test_connection`, success không save/connect. Save gọi `save_host_profile` rồi close editor, không Connect; native select được bọc, label vẫn hiển thị |
| Partial/loss | Discarded4/evicted8/incomplete/gaps tạo summary cần chú ý khi thu gọn; mở rộng còn nguyên lý do/gaps. Recovery anchor nằm ngoài summary và vẫn luôn với tới được |
| Tools/dialog | Sources/Trace/Topology mở/đóng được, không page overflow; export dùng dialog560px, focus trong modal; consent/export/Explain policy không thay |
| Reduced motion/scaling | Emulated reduced motion cho transition0s. Row32px/splitter8px giữ CSS geometry ở DPR1,25/1,5. 720×450@2 không page overflow; scrollbar ngang còn trong viewport |

## Tương phản và checklist thị giác

Palette được chốt trong `tokens.css`; thay bộ màu cũ không có bản token độc lập trong repo. Tính sRGB relative luminance trên màu CSS cuối; chỉ tính foreground/background opaque, không dùng opacity của disabled để suy độ tương phản của control đang hoạt động.

| Cặp / nền xấu nhất đang dùng | Tỉ lệ |
| --- | --- |
| Text trên selected | 9,96:1 |
| Muted trên selected | 5,52:1 |
| Trace level trên selected (level text yếu nhất) | 4,68:1 |
| Text trong search mark | 7,64:1 |
| Control border trên selected | 3,26:1 |
| Focus accent trên selected | 6,54:1 |

Đường phân vùng `--border`/`--rule` là đường trang trí; input/button và separator dùng `--control-border`. Gutter/status warning dùng màu ngữ nghĩa, accent dành cho primary, lựa chọn, focus và search. Không gradient/glow/backdrop blur; bóng chỉ popup/dialog; không card lồng nhau trong luồng đọc; font cần đọc≥12px. Hover/focus đều mở tooltip cho nút có title. Đã duyệt ảnh sơ bộ: gutter + timestamp dễ nhận biết, primary không cạnh tranh với log; tên dài được cắt ở navigator/tab và có tooltip.

Checklist mục8 chưa fully pass: ảnh loaded chưa dùng log thật đã ẩn danh; chưa có người không tham gia thiết kế duyệt; chưa chứng nhận mọi state/accessibility trên native. Không dùng third-party critique tool hoặc cài package thiết kế.

## Phép đo cuộn giới hạn

Cùng máy, cùng Chrome headless1440×900, production Vite build trước từ HEAD và build sau, cùng adapter IPC ngoài repo. 200 cached rows, message có ID/path/tiếng Việt, notices mô phỏng50 arrivals mỗi100ms; cuộn bằng requestAnimationFrame trong2,4s,145 frame intervals/run, ba lượt luân phiên trước/sau. Đây là workload synthetic nhỏ; cuộn chủ động pause viewport trong khi metadata arrivals tiếp tục cập nhật, không đo live render15k events/s.

| Run | Trước p95 frame interval | Sau p95 | Trước used JS heap | Sau used JS heap |
| --- | --- | --- | --- | --- |
| 1 | 16,7ms | 16,7ms | 9.408.129B | 10.632.062B |
| 2 | 16,8ms | 16,8ms | 19.597.371B | 18.394.033B |
| 3 | 16,8ms | 16,8ms | 9.124.625B | 16.912.865B |

Không ép GC; heap samples biến thiên và không phải retained heap/process-tree RSS. Không thấy regression frame interval trong probe này; chưa kết luận memory/performance pass. Font assets thêm183.717B local WOFF2; CSS output giảm khoảng47,91→38,42kB. Cần lặp lại với nguồn thật, cache2k rows, sustained updates, native WebView và RSS trước khi chốt performance gate. Tô token message tiếp tục deferred.

## Commands và phần chưa chạy

Đã PASS:

```bash
npm run build
PKG_CONFIG_PATH=/tmp/tailpro-phase6-build-deps/usr/lib/x86_64-linux-gnu/pkgconfig \
  cargo build -p tailpro-desktop --features custom-protocol --locked
git diff --check
```

`npm run build` gồm `tsc --noEmit`. Desktop dùng dependencies đã extract từ đợt trước, không cài gói hoặc daemon mới. Không đổi Rust source/DTO nên không regenerate contracts hoặc chạy engine tests. Kiểm tra local docs links/JSON và font inventory riêng, không tạo automated test trong repo.

Còn mở: loaded native bốn kích thước + Inspector, real anonymized logs và host/path/stack matrix, native clipboard và zoom/scaling/accessibility, full transient states và SSH cancellation trên host thật, người duyệt độc lập, sustained performance/RSS. Các runtime/release gates phase1–6 trước đó vẫn riêng.
