# Worklog

本地优先的 Windows 工作记录桌面应用：四象限任务、事件时间线、番茄钟、工作想法与 Obsidian Markdown 汇总。

## 当前里程碑：M8 集中设置

- React + TypeScript + Vite 界面；左侧入口收敛为“我的一天 / 随笔 / Obsidian / 设置”。
- Tauri 2 + Rust Windows 桌面层，SQLite 保存全部结构化数据。
- 四象限任务、两级子任务、每日编号和永久 ID。
- 绑定任务的完整番茄钟与休息生命周期，支持后台计时、系统通知和托盘运行。
- 独立设置界面集中管理番茄钟、本地 SQLite 数据目录、Obsidian 工作区和日记根目录。
- 本地数据库支持迁移并即时切换，新位置生效时保留原数据库作为安全备份。
- 可选择本地 Obsidian Vault，安全预览、编辑并同步 Markdown。
- 日记根目录由用户在 Obsidian 工作区内指定，输出 `YYYY/YYYY-MM/YYYY-MM-DD.md`，只替换受管理区块。
- 工作记录按回顾等级投影，隐藏草稿与机械事件，严格控制噪音。
- 日终收尾只顺延未完成任务，次日重新编号且永久 ID 不变。
- Windows NSIS 当前用户安装包，无需管理员权限。
- 安装包内置 WebView2 离线运行时，可在无网络环境完成依赖安装。
- 中文/英文安装界面、正式应用图标和 SHA-256 下载校验文件。
- Git 标签自动构建并发布 GitHub Release；PR 会实际构建安装包作为冒烟验证。

## 下载与安装

从仓库的 [Releases](https://github.com/yhyyds/worklog/releases) 下载最新的 `*-setup.exe` 和 `SHA256SUMS.txt`，校验后双击安装。当前安装包尚未配置商业代码签名，Windows SmartScreen 可能显示“未知发布者”；详细说明见 [M7 Windows 发行说明](docs/M7_WINDOWS_RELEASE.md)。

## 本地开发

前置条件：Node.js 24、Rust stable，以及 Tauri 2 的 Windows 系统依赖。

```bash
npm install
npm run dev
```

桌面模式与完整检查：

```bash
npm run tauri dev
npm run check
cargo test --manifest-path src-tauri/Cargo.toml
```

构建 Windows 安装包：

```bash
npm run bundle:windows
```

## 数据原则

- SQLite 是任务、番茄钟和时间线的内部事实源。
- 原始事件完整保留，日记读取降噪后的回顾投影。
- Obsidian 只作为 Markdown 查看、补充与回顾入口。
- 自动生成内容只写入受管理区块，保留人工补充。
- 安装、升级与卸载流程不得主动删除用户数据库或 Obsidian 文件。

详细设计见 [架构基线](docs/ARCHITECTURE.md)、[产品基线](docs/PRODUCT_BASELINE.md)、[M3 Obsidian 同步](docs/M3_OBSIDIAN_SYNC.md)、[M4 日终收尾](docs/M4_DAY_CLOSING.md)、[M5 笔记工作区](docs/M5_NOTES_WORKSPACE.md)、[M6 番茄钟生命周期](docs/M6_FOCUS_LIFECYCLE.md)和 [M7 Windows 发行](docs/M7_WINDOWS_RELEASE.md)。
