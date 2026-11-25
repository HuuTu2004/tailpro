# TailPro

Ứng dụng desktop đọc và kiểm tra log trên server qua SSH, viết bằng Rust, Tauri và React. [English](README.md).

[Tải bản phát hành](https://github.com/HuuTu2004/tailpro/releases) · [Trạng thái build](https://github.com/HuuTu2004/tailpro/actions) · [Bằng chứng nghiệm thu](docs/process/current-status.md)

Bản 0.1.0 là development preview. Mỗi gói được build trên hệ điều hành đích; các trường hợp chưa nghiệm thu được ghi rõ trong tài liệu trạng thái.

## Cài đặt

| Máy dùng ứng dụng | Gói tải | Yêu cầu |
| --- | --- | --- |
| Windows x64 | Setup `.exe` | Windows 10/11, WebView2, Windows OpenSSH Client |
| Mac Apple Silicon | `aarch64.dmg` | macOS 12 trở lên |
| Mac Intel | `x64.dmg` | macOS 12 trở lên |
| Linux x64 | `.deb` / `.AppImage` | Ubuntu 24.04 hoặc desktop tương thích GTK 3, WebKitGTK 4.1, OpenSSH; AppImage có thể cần FUSE 2 |

Gói chưa có chứng thư ký Apple Developer/Windows. Mac dùng ad-hoc signing, chưa notarize nên Gatekeeper có thể chặn mở; Windows có thể hiện SmartScreen. Kiểm tra SHA256SUMS.txt và dùng thao tác cho phép rõ ràng của hệ điều hành nếu bạn tin nguồn tải. Chưa có tự cập nhật.

Server đích cần Linux/macOS, POSIX shell và Python 3. Docker cần CLI, daemon và quyền đọc log. Journal cần Linux/systemd và quyền truy cập. Chưa hỗ trợ đọc log server Windows. Tailscale chỉ cung cấp đường mạng SSH, ứng dụng không quản lý VPN.

**Trên Windows:** dùng SSH key hoặc Windows OpenSSH agent. Nạp key có passphrase vào agent trước khi kết nối. Windows OpenSSH chưa hỗ trợ ControlMaster, nên các reader dùng tiến trình SSH riêng. Password/keyboard-interactive hiện dùng trên client Linux/macOS; ứng dụng báo rõ nếu chọn phương thức này trên Windows.

## Sử dụng

1. Mở Connections, nhập host hoặc SSH alias, user và phương thức xác thực rồi Connect. Host mới cần xác minh fingerprint với nguồn tin cậy.
2. Lần đầu kết nối sẽ mở **Open logs**: chọn **Found sources**, **Browse folders** hoặc **Enter path**. Có thể chọn nhiều nguồn rồi **Open selected**.
3. Viewer có đúng ba tùy chọn **Columns**, **Time** và **Status**. Time chọn ngày/giờ; Status lọc mức log. Search, paging và export dùng cùng query backend.
4. Server, đường dẫn nguồn và phạm vi quét được nhớ lại. Bấm host đã lưu để kết nối lại; bật **Restore previous readers after I reconnect** nếu muốn khôi phục reader. Password, OTP và nội dung private key không lưu trong settings.
5. Dùng Settings để cấp quyền MCP hoặc cấu hình Explain Error khi cần. Export có bước xem trước dữ liệu đã redaction.

Nút + cạnh host mở bộ chọn log. Quét chỉ đọc metadata có giới hạn tại các vị trí hiển thị; thêm folder để mở rộng phạm vi. Không cam kết tự tìm toàn bộ log trên filesystem. File nén hiển thị không được hỗ trợ. App giữ log trong RAM, hiển thị giới hạn lưu giữ và khoảng trống continuity. Tối đa 3 host và 8 reader.

## Build từ source

Cài [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) cho hệ điều hành đang dùng, Rust 1.97.1 (đã pin trong rust-toolchain.toml), Node.js 22.12+ và npm:

```bash
npm ci
npm run contracts
npm run tauri dev
```

Build gói cài đặt trên đúng hệ điều hành:

```bash
npm run tauri build -- --locked
cargo build -p tailpro-mcp --release --locked
```

Gói nằm trong `target/release/bundle/`. Workflow build Windows x64, Linux x64, Mac ARM64 và Mac Intel; chỉ publish release có tag sau khi tất cả build thành công. Adapter MCP được phát hành riêng theo từng nền tảng; dùng đường dẫn tuyệt đối tới executable trong cấu hình enrollment ở Settings. Không chia sẻ token enrollment.

Phím tắt: Ctrl/Cmd+F tìm kiếm; Space ở bảng để pause/resume; Enter mở chi tiết; Escape đóng popup/editor.

## Tài liệu và giới hạn nghiệm thu

Xem [CONTRIBUTING](CONTRIBUTING.md), [thiết kế kết nối/chọn nguồn](docs/system-design/connection-and-source-picker.md), [kiến trúc](docs/system-design/architecture.md) và [trạng thái thực tế](docs/process/current-status.md). Đã có bằng chứng đọc log Java/Spring Boot trên Mac qua Tailscale từ client Linux. Build package không thay thế nghiệm thu runtime Windows/Mac desktop hoặc các gate tải lớn còn mở.

MIT license. Font IBM Plex giữ license kèm theo trong source.
