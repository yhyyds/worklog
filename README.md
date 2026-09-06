# Worklog

本地优先的 Windows 工作记录桌面应用：四象限任务、事件时间线、番茄钟、工作想法与 Obsidian Markdown 汇总。

## 当前里程碑：1.1 分类规划与隐私分享

当前本机分支为 `codex/1.1.0-private-category-planning`，版本 `1.1.0`。任务可编辑、删除、按日安排并设置重要程度与紧急程度；打卡与目标支持自建彩色分类；周报按分类统计，并提供隐私分享及导出预览。使用说明与边界见 [1.1 说明](docs/V1_1_PLANNING_PRIVACY.md)。真实周期自动轮转与引语库扩充仍在待办中。按最新约定，保留自动化测试和编译检查，界面由用户试用反馈，不再每轮执行完整 demo。

- 待办箱：无日期收集、现有任务收纳和父子任务组重新排期，保留永久 ID 与原始记录。

- React + TypeScript + Vite 界面；“我的一天”始终作为主页面，随笔、Obsidian 与设置使用弹窗。
- 父子任务均可修改内容与当天时间段；四象限支持独立滚动、快捷新增和完成项置底。
- Tauri 2 + Rust Windows 桌面层，SQLite 保存全部结构化数据。
- 四象限任务、两级子任务、每日编号和永久 ID。
- 绑定一级任务的完整番茄钟与休息生命周期，支持后台计时、系统通知和托盘运行；暂停必须记录原因，任务切换写入时间线。
- Obsidian 日记把每轮专注写成父级 Markdown 列表，时间轴事件使用制表位缩进，直接依靠 Obsidian 原生嵌套列表折叠。
- 独立设置界面集中管理番茄钟、本地 SQLite 数据目录、Obsidian 工作区和日记根目录。
- 本地数据库支持迁移并即时切换，新位置生效时保留原数据库作为安全备份。
- 可选择本地 Obsidian Vault，安全预览、编辑并同步 Markdown。
- 日记根目录由用户在 Obsidian 工作区内指定，输出 `YYYY/YYYY-MM/YYYY-MM-DD.md`，只替换受管理区块。
- 工作记录按回顾等级投影，隐藏草稿与机械事件，严格控制噪音。
- 日终收尾只顺延未完成任务，次日重新编号且永久 ID 不变。
- Windows NSIS 当前用户安装包，无需管理员权限；同时提供不内置 WebView2 的小体积版和内置 WebView2 离线运行时的完整离线版。
- 中文/英文安装界面、正式应用图标和 SHA-256 下载校验文件。
- Git 标签自动构建并发布 GitHub Release；PR 会实际构建安装包作为冒烟验证。
- 主窗口关闭行为可在“直接退出”与“隐藏到托盘”之间切换，策略由桌面后端执行。
- 历史记录支持选择任意过去日期，查看任务、时间线、日记状态并重新生成指定日期的 Obsidian 日记。
- Obsidian 文件临时占用时自动短暂重试；失败状态持久化，之后可以安全重试，原文和备份不丢失。
- 专注支持倒计时与正向计时，正向计时由手动完成事件结束并准确排除暂停时间。
- “成长”工作区支持次日到工位后结算昨日打卡、前置依赖、有效完成判定和长期目标滚动规划。
- 长期目标包含自定义周期、阶段随想、重复/一次性与必须/附加计划，允许超过 100% 并显示铜、银、金成就。
- “一周小笺”使用个人历史基准生成任务、专注、打卡和目标报告，附带可核验出处的离线引语并可导出 PNG 长图。

## 下载与安装

从仓库的 [Releases](https://github.com/yhyyds/worklog/releases) 下载对应安装包和 `SHA256SUMS.txt`：

- `*-no-webview2-setup.exe`：体积较小，适合已安装 WebView2 Runtime 的 Windows 10/11 电脑；安装包不会下载或安装 WebView2。
- `*-with-webview2-setup.exe`：内置 WebView2 离线运行时，体积较大，适合离线电脑或无法确定运行时是否存在的环境。

校验后双击安装。当前安装包尚未配置商业代码签名，Windows SmartScreen 可能显示“未知发布者”；详细说明见 [M7 Windows 发行说明](docs/M7_WINDOWS_RELEASE.md)。

## 本地开发

前置条件：Node.js 24、Rust stable，以及 Tauri 2 的 Windows 系统依赖。

```bash
npm ci
npm run dev
```

桌面模式与完整检查：

```bash
npm run tauri dev
npm run check
cargo test --locked --manifest-path src-tauri/Cargo.toml
```

一次构建两份 Windows 安装包并生成 SHA-256 清单：

```bash
npm run bundle:windows
```

产物保存在 `artifacts/windows/`，文件名明确包含 `no-webview2` 或 `with-webview2`。

## 数据原则

- SQLite 是任务、番茄钟和时间线的内部事实源。
- 原始事件完整保留，日记读取降噪后的回顾投影。
- Obsidian 只作为 Markdown 查看、补充与回顾入口。
- 自动生成内容只写入受管理区块，保留人工补充。
- 安装、升级与卸载流程不得主动删除用户数据库或 Obsidian 文件。

详细设计见 [M9 使用细节](docs/M9_USAGE_REFINEMENTS.md)、[架构基线](docs/ARCHITECTURE.md)、[产品基线](docs/PRODUCT_BASELINE.md)、[M3 Obsidian 同步](docs/M3_OBSIDIAN_SYNC.md)、[M4 日终收尾](docs/M4_DAY_CLOSING.md)、[M5 笔记工作区](docs/M5_NOTES_WORKSPACE.md)、[M6 番茄钟生命周期](docs/M6_FOCUS_LIFECYCLE.md)和 [M7 Windows 发行](docs/M7_WINDOWS_RELEASE.md)。
