# Worklog

本地优先的 Windows 工作记录桌面应用：四象限任务、事件时间线、番茄钟、工作想法与 Obsidian Markdown 汇总。

## 当前里程碑：M2 SQLite 核心

- React + TypeScript + Vite 界面。
- Tauri 2 + Rust Windows 桌面层。
- SQLite 管理桌面端任务、事件、工作想法和番茄钟。
- 每个写操作在同一事务中更新状态并追加不可变事件。
- 浏览器开发模式使用 localStorage 适配器，桌面模式使用 SQLite 适配器。
- 四象限任务、两级子任务、每日编号和永久 ID。
- 绑定任务的番茄钟，支持暂停、恢复、切换和结束。
- Windows CI 同时运行前端检查与 Rust 事务测试。

## 本地开发

前置条件：Node.js 24、Rust stable，以及 Tauri 2 的 Windows 系统依赖。

```bash
npm install
npm run dev
```

桌面模式：

```bash
npm run tauri dev
```

完整检查：

```bash
npm run check
cargo test --manifest-path src-tauri/Cargo.toml
```

## 数据原则

- SQLite 是任务、番茄钟和时间线的内部事实源。
- 原始事件完整保留，日记读取降噪后的回顾投影。
- Obsidian 只作为 Markdown 查看、补充与回顾入口。
- 自动生成内容只写入受管理区块，保留人工补充。

详细设计见 [架构基线](docs/ARCHITECTURE.md)、[产品基线](docs/PRODUCT_BASELINE.md) 和 [M2 实现说明](docs/M2_IMPLEMENTATION.md)。
