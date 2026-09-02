# M7：Windows 正式发行

## 目标

M7 将 Worklog 从“可开发运行的桌面应用”推进为“可下载、可验证、可直接安装的 Windows 产品”。业务数据仍完全保存在本地，发行流程不引入云端账户或遥测。

## 安装包决策

| 项目 | 决策 |
| --- | --- |
| 格式 | NSIS `*-setup.exe` |
| 架构 | Windows x64 |
| 安装范围 | 当前用户，无需管理员权限 |
| WebView2 | 离线安装程序内置，安装包约增加 127 MB |
| VC 运行时 | Rust/Tauri 使用静态 VC Runtime |
| 安装语言 | 简体中文、英文；自动跟随系统语言 |
| 数据迁移 | SQLite 启动时执行幂等迁移 |
| 下载校验 | Release 同步发布 `SHA256SUMS.txt` |
| 代码签名 | 暂未配置；后续接入证书后消除未知发布者提示 |

选择离线 WebView2 是为了让安装不依赖目标电脑的网络环境。相比默认的在线引导程序，代价是安装包明显增大。

## 用户安装流程

1. 从 GitHub Release 下载 `Worklog_*_x64-setup.exe` 与 `SHA256SUMS.txt`。
2. 在 PowerShell 运行 `Get-FileHash .\Worklog_*_x64-setup.exe -Algorithm SHA256`。
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
5. `Windows Release` 工作流验证标签版本、运行全部测试、构建安装包、创建 Release 并上传 SHA-256 校验文件。

标签版本与应用版本不一致时，工作流会立即失败，不会发布错误版本。

## 维护边界

- GitHub Release 只承载构建产物，不接收应用内数据。
- 安装包不会预设 Obsidian 路径。
- 日志数据库和 Markdown 不进入安装包或 CI artifact。
- 暂不启用应用内自动更新；这需要单独的签名密钥管理与回滚方案。
- 私有仓库的 Release 仅对有仓库访问权限的账号可见；对外分发时可由所有者另行提供安装包，或后续建立公开的二进制发布仓库。
