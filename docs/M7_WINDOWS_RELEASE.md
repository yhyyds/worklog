# M7：Windows 正式发行

## 目标

M7 将 Worklog 从“可开发运行的桌面应用”推进为“可下载、可验证、可直接安装的 Windows 产品”。业务数据仍完全保存在本地，发行流程不引入云端账户或遥测。

## 安装包决策

| 项目 | 决策 |
| --- | --- |
| 格式 | 两份 NSIS `*-setup.exe`，另提供 no-WebView2 WiX MSI 供企业部署 |
| 架构 | Windows x64 |
| 安装范围 | 当前用户，无需管理员权限 |
| WebView2 | 同时提供不内置的小体积版，以及内置离线运行时、约增加 127 MB 的离线版 |
| VC 运行时 | Rust/Tauri 使用静态 VC Runtime |
| 安装语言 | 简体中文、英文；自动跟随系统语言 |
| 数据迁移 | SQLite 启动时执行幂等迁移 |
| 下载校验 | Release 同步发布 `SHA256SUMS.txt` |
| 代码签名 | 暂未配置；后续接入证书后消除未知发布者提示 |

`no-webview2` 安装包不检查、下载或安装 WebView2，目标电脑必须已有运行时；`with-webview2` 安装包内置离线运行时，让安装不依赖目标电脑的网络环境，代价是安装包明显增大。

此外构建 `Worklog_*_x64-no-webview2.msi`。MSI 不是绕过 SmartScreen 的工具，而是便于公司 IT 通过 Windows Installer、软件中心、Intune、组策略或文件哈希白名单审核部署。仓库未配置可信代码签名时，NSIS 和 MSI 都可能被组织安全策略阻止。

## 用户安装流程

1. 根据环境下载 `Worklog_*_x64-no-webview2-setup.exe` 或 `Worklog_*_x64-with-webview2-setup.exe`，并下载 `SHA256SUMS.txt`。
2. 在 PowerShell 运行 `Get-FileHash .\Worklog_*_x64-*-setup.exe -Algorithm SHA256`。
3. 将输出与 `SHA256SUMS.txt` 对照，并查看 `BUILD-INFO.txt` 中的提交与 Authenticode 状态。
4. 个人电脑优先使用 NSIS；公司电脑若拦截未知 EXE，把 MSI、校验文件和构建信息提交给 IT 审核部署。
5. 首次启动后选择 Obsidian Vault。软件不会自动扫描其他目录。

Windows SmartScreen 可能因安装包尚未使用受信任证书签名而提示未知发布者。SHA-256 只能证明下载文件与构建产物一致，不能建立发行者身份或替代组织安全策略。不得提供关闭 Defender、移除 Mark-of-the-Web 或绕过公司策略的脚本；正式解决方案仍是可信代码签名或 IT 白名单。

## 自动发行

版本只在以下三个文件中声明，并由 `scripts/check-version.mjs` 强制保持一致：

- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

标准发布步骤：

1. 同步更新三处版本与 `CHANGELOG.md`。
2. 通过 PR 的前端、Rust、双 NSIS 与 no-WebView2 MSI 验证。
3. 合并到 `main`。
4. 在合并提交上创建 `vX.Y.Z` 标签。
5. `Windows Release` 工作流验证标签版本、运行全部测试，构建两份 NSIS 和一份 MSI，并上传安装包、SHA-256 与构建信息。

标签版本与应用版本不一致时，工作流会立即失败，不会发布错误版本。

## 维护边界

- GitHub Release 只承载构建产物，不接收应用内数据。
- 安装包不会预设 Obsidian 路径。
- 日志数据库和 Markdown 不进入安装包或 CI artifact。
- 暂不启用应用内自动更新；这需要单独的签名密钥管理与回滚方案。
- 公开仓库的 Release 可被任何人下载；发布前必须确认安装包、校验文件和发行说明均不包含用户数据库、Vault 或个人绝对路径。
