# Quy chuẩn compile nhanh

Workspace đã có manifests/code; các lệnh scoped dưới đây áp dụng cho vòng lặp phase 1. Mục tiêu là feedback nhanh có đủ kiểm chứng, không phải bỏ diagnostics hoặc build cần thiết.

## Vòng lặp nhỏ nhất

| Thay đổi | Làm gì |
| --- | --- |
| Docs | Kiểm tra links/nội dung, không build app |
| Rust internals | cargo check -p crate --lib; scoped clippy |
| Public Interface/DTO | Check crate và reverse dependents; regenerate bindings chỉ khi schema đổi |
| UI-only | Dev server/typecheck theo scripts thật; không rebuild Rust không liên quan |
| Cần chạy/link | Build actual binary/package; check không thay codegen/link validation |
| Integration/release | Workspace/build artifact cần phát hành, --locked và profile/features đã chốt |

```bash
cargo check -p log-core --lib
cargo check -p app-core --lib
cargo clippy -p log-core --lib -- -D warnings
cargo build --workspace --locked
cargo build -p tailpro-desktop --features custom-protocol --timings
```

Chọn command theo thay đổi, không chạy toàn bộ mỗi lượt. src-tauri là directory, không mặc định là package name. CLI/frontend scripts chỉ dùng khi manifest đã có.

## Dev profile ở workspace root

```toml
[profile.dev]
opt-level = 0
debug = "line-tables-only"
incremental = true
lto = false
```

Giữ assertions/overflow checks, đổi full symbols khi cần debugger. Giữ codegen-units mặc định trước khi đo; không bật LTO/one codegen unit trong dev theo phỏng đoán. Release profile riêng cho runtime measurements/distribution.

## Cache và dependency

Giữ target/dependency/package caches, toolchain/lockfile/flags ổn định; không cargo clean/xóa lockfile/reinstall theo thói quen. CI cache bound OS/build base/compiler/target/lockfile/config, không tin cache như bằng chứng checks đã pass. Một Cargo coordinator cho target directory, jobs phù hợp CPU/RAM tránh swap/OOM.

Giữ log-core độc lập UI/runtime SDK; phase 3 mới thêm protocol/MCP, inference chỉ khi triển khai mục 3.6. Features đúng nhu cầu, không all-features mặc định. Không thêm crate/derive/codegen framework chỉ để hy vọng compile nhanh. Linker/cache bổ sung chỉ khi timings chứng minh có lợi; sccache và incremental phải kiểm tra compatibility thay vì mặc định cộng lợi ích.

## Build script và measurements

build.rs theo dõi inputs/env thật bằng rerun-if-changed/rerun-if-env-changed, outputs ở OUT_DIR. DTO export chỉ write khi bytes đổi, không timestamp/random output hoặc watch generated/target/node_modules làm rebuild loop. Giữ platform-required hooks.

Đo cold build trong target directory riêng, no-change warm build, edit parser, edit DTO, UI-only và desktop link/packaging. Ghi environment/profile/features/jobs/cache/versions, repeated wall time và timings. Giữ cải tiến có lợi ổn định; không hứa số giây chưa đo. Không viết automated tests; dùng compile/lint/manual smoke/profiling.

Nguồn: [Cargo check](https://doc.rust-lang.org/cargo/commands/cargo-check.html), [profiles](https://doc.rust-lang.org/cargo/reference/profiles.html), [build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html), [sccache](https://github.com/mozilla/sccache/blob/main/docs/Rust.md).
