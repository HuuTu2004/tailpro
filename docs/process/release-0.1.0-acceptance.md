# TailPro0.1.0 — native release acceptance

25/11/2026. [Public repository](https://github.com/HuuTu2004/tailpro), [release](https://github.com/HuuTu2004/tailpro/releases/tag/v0.1.0), [successful build/publish run](https://github.com/HuuTu2004/tailpro/actions/runs/45571287865). Release source commit `35798b851c1b99f1760749af07cb95cc90928ed7`; subsequent main commit adds only the user-supplied screenshot to both READMEs.

| Target | Native builder | Published desktop package |
| --- | --- | --- |
| Windows x64 | windows-latest / MSVC | TailPro_0.1.0_x64-setup.exe |
| Linux x64 | ubuntu-24.04 | TailPro_0.1.0_amd64.deb, TailPro_0.1.0_amd64.AppImage |
| macOS ARM64 | macos-15 | TailPro_0.1.0_aarch64.dmg |
| macOS x64 | macos-15-intel | TailPro_0.1.0_x64.dmg |

All four builders passed formatting, core/MCP/platform Clippy with warnings denied, generated-contract consistency, frontend typecheck/build, native bundling and release MCP compilation. Four matching MCP ZIPs, five desktop packages and SHA256SUMS.txt are published. Downloaded all nine payloads from the actual public release and verified every checksum successfully. Rust toolchain is pinned to1.97.1; dependency lockfiles are committed.

Linux AppImage was extracted and launched locally with isolated empty settings/runtime. The bundled UI rendered, the process remained alive and its private MCP registry was created. Debian package metadata and MCP ZIP contents were inspected. The ARM64 DMG was copied to the Mac acceptance host, mounted read-only, checked using codesign --verify --deep --strict, identified as Mach-O arm64 and unmounted successfully. No Apple Developer notarization or Windows Authenticode certificate is configured.

Connection/source acceptance used the native Linux app and a real SSH/Tailscale macOS server with the previously authorized Spring Boot simulation. Capability results separated ready File, unavailable Docker and unsupported Journal. Scoped discovery returned the three live files and older-file hints; metadata browsing completed without reasons; opening three selected files reached streaming and saved stable workspace/source references. Main received source-open events and showed the combined scope. No host/project path is hardcoded into discovery defaults.

A local ephemeral SSH protocol server exercised the real system OpenSSH client and native attempt/challenge commands: new host fingerprint matched the independently generated server key; wrong fingerprint was rejected; verified fingerprint proceeded; two keyboard-interactive secret prompts completed authentication. Test connection finished without saving its profile or replacing the active host. Probe credentials were absent from settings; the ephemeral server and only its app-owned trust file were removed. This checks the client challenge flow, not compatibility with every production SSH/MFA provider. A native synchronous-command reactor panic and a missing OpenSSH askpass-hint assumption were corrected before the published build.

The user requested removal of the extra host-tree ellipsis/Connect buttons. The published UI retains one + beside each host; clicking a saved disconnected host opens reconnect. Both READMEs contain exactly one application screenshot: the image supplied by the user, unchanged.

## Limits

This is a preview package release, not full acceptance of historical phase gates. Windows native reader authentication requires key/Windows OpenSSH agent because its client has no ControlMaster; interactive/password sessions require a Unix client. Windows/macOS desktop install/connect/long-session matrices were not manually certified in this session. Mac signature verification is not a GUI runtime check. Sustained-load, provider/reconnect, full SSH configuration and directory permission/race matrices remain tracked in current-status.md. Config/PID advanced discovery remains available in the backend but is not added as extra picker controls. No automated tests or test infrastructure were added.
