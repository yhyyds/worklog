# M7：Windows 正式发行

## 目标

M7 将 Worklog 从“可开发运行的桌面应用”推进为“可下载、可验证、可直接安装的 Windows 产品”。业务数据仍完全保存在本地，发行流程不引入云端账户或遥测。

## 安装包决策

| 项目 | 决策 |
| --- | --- |
| 格式 | NSIS `*-setup.exe` |
| 架构 | Windows x64 |
| 安装范围 | 当前用户，无需管理员权限 |
| WebView2 | 同时提供不内置的小体积版，以及内置离线运行时、约增加 127 MB 的离线版 |
| VC 运行时 | Rust/Tauri 使用静态 VC Runtime |
| 安装语言 | 简体中文、英文；自动跟随系统语言 |
| 数据迁移 | SQLite 启动时执行幂等迁移 |
| 下载校验 | Release 同步发布 `SHA256SUMS.txt` |
| 代码签名 | 暂未配置；后续接入证书后消除未知发布者提示 |

`no-webview2` 安装包不检查、下载或安装 WebView2，目标电脑必须已有运行时；`with-webview2` 安装包内置离线运行时，让安装不依赖目标电脑的网络环境，代价是安装包明显增大。

## 用户安装流程

1. 根据环境下载 `Worklog_*_x64-no-webview2-setup.exe` 或 `Worklog_*_x64-with-webview2-setup.exe`，并下载 `SHA256SUMS.txt`。
2. 在 PowerShell 运行 `Get-FileHash .\Worklog_*_x64-*-setup.exe -Algorithm SHA256`。
3. 将输出与 `SHA256SUMS.txt` 对照。
4. 双击安装包；安装范围为当前 Windows 用户。
5. 首次启动后选择 Obsidian Vault。软件不会自动扫描其他目录。

Windows SmartScreen 可能因安装包尚未使用商业证书签名而提示未知发布者。这不影响哈希校验或程序运行，但对外公开分发前应完成代码签名。

## 自动发行

版本只在以下三个文件中声明，并由 `scripts/check-version.mjs` 强制保持一致：

- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

标准发布步骤：

1. 同步更新三处版本与 `CHANGELOG.md`。
2. 通过 PR 的前端、Rust 和双 NSIS 安装包验证。
3. 合并到 `main`。
4. 在合并提交上创建 `vX.Y.Z` 标签。
5. `Windows Release` 工作流验证标签版本、运行全部测试、真实构建两份安装包、创建 Release，并上传两份安装包和 SHA-256 校验文件。

标签版本与应用版本不一致时，工作流会立即失败，不会发布错误版本。

## 维护边界

- GitHub Release 只承载构建产物，不接收应用内数据。
- 安装包不会预设 Obsidian 路径。
- 日志数据库和 Markdown 不进入安装包或 CI artifact。
- 暂不启用应用内自动更新；这需要单独的签名密钥管理与回滚方案。
- 公开仓库的 Release 可被任何人下载；发布前必须确认安装包、校验文件和发行说明均不包含用户数据库、Vault 或个人绝对路径。
