# Worklog

本地优先的 Windows 工作记录桌面应用：四象限任务、事件时间线、番茄钟、工作想法与 Obsidian Markdown 汇总。

## 当前里程碑：M1 工程骨架

本次基线包含：

- React + TypeScript + Vite 前端，可直接在浏览器运行交互原型。
- Tauri 2 + Rust 桌面壳与 SQLite 初始化。
- 四象限任务、新建/完成任务、工作想法、低噪声时间线。
- 绑定具体任务的基础番茄钟，支持暂停、继续与提前结束。
- 每日显示编号与永久 ID 分离的领域模型。
- Windows CI：前端类型检查、测试、生产构建与 Rust 检查。

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
cargo check --manifest-path src-tauri/Cargo.toml
```

## 数据原则

- 任务、番茄钟和时间线由应用数据库管理。
- 所有状态变化追加不可变事件，日记由事件投影生成。
- Obsidian 只作为 Markdown 查看、补充与回顾入口。
- 自动生成内容只写入受管理区块，保留人工补充。

详细设计见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) 与 [docs/PRODUCT_BASELINE.md](docs/PRODUCT_BASELINE.md)。
