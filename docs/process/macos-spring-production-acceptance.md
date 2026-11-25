# macOS / Spring Boot qua Tailscale — nghiệm thu thủ công

Ngày 25/11/2026, Asia/Bangkok. Phạm vi: runtime thật của mục1.2,1.3,1.6,1.7 và Java multiline6.6; không thay thế các release/performance gates trước đó. Không viết automated tests hoặc thêm fixtures vào runtime/repo.

## Môi trường và ứng dụng

- TailPro native Linux, bundled frontend `tauri://localhost`, OpenSSH qua Tailscale tới `admins-mac-studio` (`100.115.254.123`), user `admin`; dùng existing OS SSH agent và strict host keys.
- Project ở `/Users/admin/Work/tailpro-spring-production-20261125`: Java21, Spring Boot3.5.16, embedded Tomcat, Maven wrapper3.9.11 có sẵn. Không cài Maven/Java/agent lên Mac.
- API bind `127.0.0.1:18081`; health UP. Profile `production-simulation`, heap64–256MiB. Endpoint đặt hàng thật chạy các kịch bản success, slow350ms, out-of-stock409, payment-timeout503 và Unicode. Payment/inventory là lỗi mô phỏng, không gọi dịch vụ tài chính/DB thật.
- Logback tạo `logs/application.log` (Spring headers/Java stack), `logs/audit.log` (JSON) và `logs/jobs.log` (scheduled worker10s). Size rotation128KB; archive cap8/4/2MB. Traffic nền3600requests, khoảng1request/s; app để chạy cho user xem. `README.txt` và `stop.py` trên Mac ghi cách dừng/chạy lại.

## Lỗi tái hiện và sửa

1. File helper thất bại trước handshake, hai lần tái hiện trả `FileNotFoundError: /proc/self/fd/3` trên Mac. Linux giữ descriptor reopen qua proc; macOS reopen selected path NOFOLLOW/NONBLOCK rồi so sánh regular type và device/inode với descriptor đang giữ. Recheck quyền ở idle, bounded alarm1s. Python helper gửi qua SSH, không cài làm service.
2. Logback in `... 54 common frames omitted` sau suppressed exception; Java assembler chỉ nhận `... N more`, nên group56lines thiếu `Caused by`. Thêm continuation Logback; native Inspector sau sửa giữ59lines, root `IllegalStateException`, suppressed exception và cause `SocketTimeoutException`, ~5420bytes. Header ERROR chứa request ID và exception group không timestamp vẫn là hai events riêng; lọc request ID không tự nối context không có ID.

Manual probes trên cả Linux và Mac: mở/stream PASS; rename rotation đọc được cả file cũ/mới và phát marker; symlink và FIFO bị từ chối; chmod000 sau open làm reader dừng với PermissionError. Các probes chạy ngoài repo với thư mục tạm; không thêm automated tests.

## Đối chiếu dữ liệu qua backend native

Hai đợt300requests; đợt sau fix dùng prefix `SIM-20261125-133454`. Expected từ HTTP generator được so với paged queries của chính app qua native IPC, không fake IPC/data hoặc frontend adapter.

| Kiểm tra | Quan sát |
| --- | --- |
| HTTP | 300requests:240×201,30×409,30×503;0transport failures |
| application `http_completed` + prefix | 300events,300request IDs riêng |
| audit JSON + prefix | 300events:210Info,60Warn,30Error;2pages |
| application prefix + `level:error` | 30events có request ID |
| application prefix + `level:warn` | 60events |
| Unicode | 30audit messages đúng `Đặt hàng thành công — cà phê ☕` |
| Java detail | Root, frames, suppressed và timeout cause cùng recognized Java group |
| Rotation | Application qua nhiều archives; audit qua ít nhất1archive; readers tiếp tục streaming |
| Query jobs | Khoảng2,3–9,0ms mỗi page trong mẫu, không phải E2E/sustained p95 |
| Reader counters sau burst | observed_bytes=represented_bytes ở cả3sources;0gap_units/gap_bytes,0malformed/encoding_loss/retention_evicted_bytes |

Rotation chủ động ghi continuity-uncertain marker và source-loss revision; `gap_sources` trong paged queries vẫn có source đã rotate. Đối chiếu đủ300request IDs chỉ chứng minh đợt traffic được quan sát, không bỏ marker hoặc tuyên bố exactly-once/upstream history đầy đủ. Discovery tìm đúng3regular files, application/audit có observed growth; overall `complete=false` vì project presets không tồn tại và macOS không có systemctl. Không coi systemd/Journal là supported trên Mac.

## Ghi nhớ kết nối qua restart

Profile đã được persist bằng SettingsStore versioned/atomic0600 trước đợt này, nhưng main HOSTS chỉ hiện active runtime hosts nên mở lại app trông như mất kết nối. Main nay merge saved profiles với active connections theo profile ID; active state luôn ưu tiên. Saved offline hosts hiển thị `Saved · Disconnected` và Connect. Thông tin host/user/port/IdentityAgent/environment được dùng lại nguyên profile; saved dropdown trong form vẫn giữ.

Native restart thực tế: saved profile=1, active connections=0, readers=0; server và Connect vẫn hiện trong HOSTS. Bấm Connect một lần kết nối thật, profile giống hệt persisted profile, không nhập lại và không tạo profile trùng. Ba pinned file paths xuất hiện lại sau xác minh remote identity. Double-click ba paths tạo3readers streaming. Chuyển từ temporary debug agent sang existing OS SSH agent làm remote identity đổi và pins cũ bị ẩn đúng policy; sau khi pin dưới identity hiện tại, restart lại cùng OS agent xác nhận cùng kết quả. Profile server vẫn được giữ qua cả thay đổi agent.

Saved profile không giả mạo trạng thái Connected. Connect tường minh; không tự cấp dynamic-config consent hoặc Start nguồn log. Pins vẫn gắn remote identity, chỉ được đọc sau kết nối. Host loading refresh theo profile changes/focus/event, không polling settings mỗi log frame. Saved disconnected host không làm footer báo Host needs attention; needs_action/reconnecting vẫn báo.

## Kiểm chứng và giới hạn

PASS: `npm run build` (typecheck/Vite), `cargo fmt --all -- --check`, scoped `cargo clippy -p log-core -p app-core --lib --locked -- -D warnings`, native bundled-assets build `cargo build -p tailpro-desktop --features custom-protocol --locked`, `git diff --check`, manual native restart/one-click reconnect/pinned paths/live queries/Inspector và Linux/macOS helper probes. Desktop build dùng pkg-config dependencies extract từ đợt trước.

Artifacts debug ở `/tmp/tailpro-spring-live/` ngoài repo: verification/discovery JSON và native screenshots. Log mẫu và private keys không đưa vào git. WebKitWebDriver điều khiển native webview và native IPC thật; không inject backend giả. Ảnh lưu kết nối `saved-connection.png`, log live `live-spring-native.png`.

Chưa chứng nhận sustained load15k/s, mất mạng dài/reconnect races, copytruncate/regrow, macOS ACL/security matrix,2hosts/4sources, cold launch/RSS hoặc release compatibility. Đây là một project Spring Boot và3file sources trên Mac thật; các phase gates còn lại vẫn PARTIAL.
