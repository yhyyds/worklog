# M7：Windows 正式发行

## 目标

M7 将 Worklog 从“可开发运行的桌面应用”推进为“可下载、可验证、可直接安装的 Windows 产品”。业务数据仍完全保存在本地，发行流程不引入云端账户或遥测。

## 安装包决策

| 项目 | 决策 |
| --- | --- |
| 格式 | NSIS `*-setup.exe` |
| 架构 | Windows x64 |
| 安装范围 | 当前用户，无需管理员权限 |
| WebView2 | 同时提供不内置运行时的轻量包与内置离线运行时的完整包 |
| VC 运行时 | Rust/Tauri 使用静态 VC Runtime |
| 安装语言 | 简体中文、英文；自动跟随系统语言 |
| 数据迁移 | SQLite 启动时执行幂等迁移 |
| 下载校验 | Release 同步发布 `SHA256SUMS.txt` |
| 代码签名 | 暂未配置；后续接入证书后消除未知发布者提示 |

轻量包使用 Tauri 的 `downloadBootstrapper` 模式，安装包不包含 WebView2；若系统缺少运行时，安装过程会联网下载。完整包使用 `offlineInstaller`，可在无网络环境安装，代价是体积明显增大。Windows 10（2018 年 4 月更新及以后）与 Windows 11 通常已随系统分发 WebView2。

## 用户安装流程

1. 从 GitHub Release 下载 `*-no-webview2-setup.exe` 或 `*-with-webview2-setup.exe`，并下载 `SHA256SUMS.txt`。
2. 在 PowerShell 运行 `Get-FileHash .\\Worklog-*-setup.exe -Algorithm SHA256`。
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
2. 通过 PR 的前端、Rust 和 NSIS 安装包验证。
3. 合并到 `main`。
4. 在合并提交上创建 `vX.Y.Z` 标签。
5. `Windows Release` 工作流验证标签版本、运行全部测试、构建两种安装包、创建 Release 并上传统一的 SHA-256 校验文件。

标签版本与应用版本不一致时，工作流会立即失败，不会发布错误版本。

## 维护边界

- GitHub Release 只承载构建产物，不接收应用内数据。
- 安装包不会预设 Obsidian 路径。
- 日志数据库和 Markdown 不进入安装包或 CI artifact。
- 暂不启用应用内自动更新；这需要单独的签名密钥管理与回滚方案。
- 私有仓库的 Release 仅对有仓库访问权限的账号可见；对外分发时可由所有者另行提供安装包，或后续建立公开的二进制发布仓库。
