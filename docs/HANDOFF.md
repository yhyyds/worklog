# Worklog 0.9.1 本地 Codex 交接

## 快照

| 项目 | 值 |
| --- | --- |
| 仓库 | `yhyyds/worklog`（private） |
| 本地迭代基线 | `0.9.1` |
| 精确提交 | `7d2fdfc203ff55358abfd7b9dfdebb5140a27fb3` |
| 交接分支 | `codex/0.9.1-local-handoff` |
| 前端 | React + TypeScript + Vite |
| 桌面后端 | Tauri 2 + Rust |
| 数据 | SQLite + Obsidian Markdown |
| Windows 安装 | NSIS，当前用户安装 |

`main` 上存在更晚的 `0.9.2` 提交，但用户明确要求重新以 `0.9.1` 为基础迭代。保留全部 Git 历史，不把 `0.9.2` 代码带入本分支。

## 0.9.1 已实现能力

- 四象限任务、两级子任务、永久 ID 与每日显示编号。
- 父子任务可编辑标题及当天计划时间；象限独立滚动、快捷新增、完成项置底。
- “我的一天”固定为主页面；随笔、Obsidian、设置使用弹窗。
- 完整专注、短休息、长休息状态机，支持后台计时、托盘、通知和全局快捷键。
- 专注只选择一级任务；暂停必须填写原因；切换任务生成可见时间线。
- 集中设置番茄钟、字号、本地数据库目录、Obsidian Vault 与日记根目录。
- SQLite 数据目录安全迁移：一致性副本验证成功后切换，旧库保留。
- Obsidian Vault 中 Markdown 浏览、搜索、编辑、预览、Wiki Link 和外部修改冲突处理。
- 日记路径：`<Vault>/<日记根目录>/YYYY/YYYY-MM/YYYY-MM-DD.md`。
- 受管理区块安全更新；标记外人工内容保留；写入前备份并原子替换。
- 日终仅顺延未完成任务；次日重新编号但永久 ID 不变。
- 任务完成标记使用固定 viewBox SVG，避免字体基线导致视觉偏移。

## 当前 0.9.1 的日记格式

`0.9.1` 使用 `<details>` / `<summary>` 包装每一轮专注时间线。该行为是下一轮需要替换的对象，不应被误写成已经完成。

## 下一轮已确认需求

1. 删除日记中的全部 `<details>` 和 `<summary>`。
2. 每轮使用一条父级 Markdown 列表记录：`第N轮任务，专注时段：HH:MM-HH:MM，任务记录：`。
3. 每条时间轴事件紧跟父级行，前面缩进一个制表位。
4. 父级行与首条子事件之间不得有空白行。
5. 依赖 Obsidian 原生嵌套列表折叠，不自行输出 HTML。
6. 打包两份 Windows NSIS：不内置 WebView2、小体积版本；内置离线 WebView2、可离线安装版本。
7. 上述实现必须从本交接分支继续，而不是使用现有 `0.9.2` 提交。

## 关键代码导航

| 范围 | 主要文件 |
| --- | --- |
| 主界面与交互 | `src/App.tsx`、`src/styles.css` |
| 前端用例 | `src/application/useWorklog.ts`、`gateway.ts` |
| 桌面 IPC | `src/infrastructure/desktopGateway.ts`、`src-tauri/src/commands.rs` |
| 浏览器开发适配 | `src/infrastructure/browserGateway.ts` |
| 领域模型 | `src/domain/model.ts`、`src-tauri/src/model.rs` |
| SQLite | `src-tauri/src/db.rs`、`src-tauri/migrations/` |
| 本地存储迁移 | `src-tauri/src/storage.rs` |
| 番茄钟 | `src-tauri/src/timer.rs` |
| Obsidian 日记 | `src-tauri/src/obsidian.rs` |
| Markdown 随笔 | `src-tauri/src/notes.rs` |
| 日终顺延 | `src-tauri/src/closing.rs` |
| Tauri 初始化 | `src-tauri/src/lib.rs` |
| CI / Release | `.github/workflows/ci.yml`、`.github/workflows/release.yml` |

## 开发与验证

前置：Windows、Node.js 24、Rust stable、Tauri 2 Windows 构建依赖。

```powershell
npm install
npm run check
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri dev
npm run bundle:windows
```

`0.9.1` 的预期基线是前端 9 项测试、Rust 26 项测试。实际结果以本机执行和 CI 为准。

## 数据位置与备份

- 数据库文件名：`worklog.db`。
- 存储位置指针：`storage-location.json`。
- 默认应用数据目录通常位于 `%APPDATA%\cn.worklog.desktop`；若用户在设置中迁移过数据库，应以设置页显示的实际数据目录为准。
- 备份前必须从系统托盘彻底退出 Worklog。
- 必须同时复制实际数据库目录、应用状态目录和整个 Obsidian Vault。
- Vault 备份必须包含 `.obsidian` 与 `.worklog-backups`。
- 复制应使用 `/E`，不要使用可能删除目标文件的 `robocopy /MIR`。

## 可复现性缺口

`0.9.1` 没有提交 `package-lock.json` 和 `src-tauri/Cargo.lock`。因此当时的精确传递依赖无法仅凭 Git 逐字节恢复。处理原则：

1. 立即保存仍可取得的 `0.9.1` 安装包及 SHA-256，作为黄金产物。
2. 在本地迁移分支生成两个锁文件。
3. 完成全部测试和 Windows 安装包构建后再提交锁文件。
4. 后续使用 `npm ci` 与 Cargo `--locked`。

## 交接验收

- HEAD 可追溯到指定 `0.9.1` 提交。
- 三处版本号均为 `0.9.1`。
- Git 镜像和 Bundle 验证成功。
- SQLite、应用状态目录、Vault 及隐藏目录均有独立备份和 SHA-256 清单。
- 本地 Codex 能复述数据安全约束与下一轮需求。
- 前端、Rust、Windows 安装包验证通过。
- 在本地完成至少一次分支提交和 PR 前，不删除旧环境或原始数据。