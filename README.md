# Worklog

本地优先的 Windows 工作记录桌面应用：四象限任务、事件时间线、番茄钟、工作想法与 Obsidian Markdown 汇总。

## 当前里程碑：M5 随笔与 Obsidian 文件工作区

- React + TypeScript + Vite 界面。
- Tauri 2 + Rust Windows 桌面层。
- SQLite 管理任务、事件、工作想法、番茄钟与同步状态。
- 四象限任务、两级子任务、每日编号和永久 ID。
- 绑定任务的番茄钟，支持暂停、恢复、切换和结束。
- 可选择本地 Obsidian Vault，预览并同步今日日记。
- 默认写入 `工作日志/YYYY/YYYY-MM/YYYY-MM-DD.md`。
- 只替换受管理 Markdown 区块，保留所有人工内容。
- 标记异常时拒绝覆盖；覆盖前备份并采用原子文件替换。
- 隐藏/草稿事件不进入日记，严格控制回顾噪音。
- 日终页面汇总已完成、等待与阻塞事项，并逐项确认顺延。
- 已完成子任务不会复制；未完成子任务可独立顺延并在必要时提升为顶级事项。
- 次日重新生成显示编号，任务永久 ID 保持不变。
- 日终确认后生成明日任务草稿，并在已配置时同步 Obsidian 今日日记。
- 直接浏览、搜索、编辑和渲染 Vault 内的 Markdown 文件。
- 随笔正文只保存在 Markdown 中，SQLite 不复制正文。
- 支持 GFM 与 Obsidian Wiki Link，外部修改可自动刷新或提示冲突。
- 新建随笔只向今日记录写入一条简洁事件，后续编辑保持隐藏。
- 路径穿越、符号链接越界、非法文件名和超大文件均受保护。
- Windows CI 同时运行前端检查与 Rust 测试。

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

详细设计见 [架构基线](docs/ARCHITECTURE.md)、[产品基线](docs/PRODUCT_BASELINE.md)、[M2 实现说明](docs/M2_IMPLEMENTATION.md) 和 [M3 Obsidian 同步](docs/M3_OBSIDIAN_SYNC.md) 和 [M4 日终收尾](docs/M4_DAY_CLOSING.md) 和 [M5 笔记工作区](docs/M5_NOTES_WORKSPACE.md)。
