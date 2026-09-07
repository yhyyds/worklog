# Worklog 仓库指令

## 唯一开发基线

- 本地迁移与后续迭代必须从 Worklog `0.9.1` 开始。
- 精确基线提交：`7d2fdfc203ff55358abfd7b9dfdebb5140a27fb3`。
- 交接分支：`codex/0.9.1-local-handoff`。
- 本机功能分支 `codex/0.9.1-native-folding-dual-installers` 从上述交接分支创建，并独立实现 `0.9.2`，不得改从远端 `0.9.2` 获取代码。
- `main` 已包含更晚的 `0.9.2`。除非用户明确批准，不得把 `main`、`b420c0c` 或 M9.2 提交合并、变基或 cherry-pick 到本分支。
- 修改前先检查 `git status --short`、当前分支、`git rev-parse HEAD`，并确认 `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` 的版本一致。

## 产品与架构

- Worklog 是 Windows 优先、本地优先的桌面应用。
- 前端：React、TypeScript、Vite。
- 桌面层：Tauri 2、Rust。
- 结构化事实源：SQLite；Obsidian Markdown 是可读投影、补充和回顾入口。
- 主页面始终是“我的一天”；随笔、Obsidian、设置只打开弹窗。
- 任务最多两级；永久 UUID 不变，每日显示编号跨日重新生成。
- 原始领域事件不可变；界面和日记读取降噪后的回顾投影。
- 专注只能绑定一级任务；暂停必须填写原因；切换任务必须生成可见事件。

## 数据安全红线

- 安装、升级、卸载、迁移、测试均不得主动删除用户数据库或 Obsidian 文件。
- 数据库迁移必须先复制、验证可打开，再切换位置；原数据库保留。
- 不得用测试库覆盖真实 `worklog.db`。
- 写 Obsidian 前必须备份并使用安全替换；只能替换受管理标记之间的内容，标记外人工文字原样保留。
- 必须拒绝目录穿越、Vault 外路径、符号链接越界和隐藏备份目录误枚举。
- 改动持久化结构时必须提供向前兼容迁移和回归测试。

## 0.9.1 基线与本机 0.9.2 行为

- `0.9.1` 的番茄钟日记仍使用 `<details>` / `<summary>` 折叠区块，这是当前基线事实。
- 本机 `0.9.2` 分支移除上述 HTML；父级时间段记录后立即跟随子事件，子事件缩进一个制表位，中间没有空白行，让 Obsidian 使用原生列表折叠。
- 本机 `0.9.2` 分支输出两份 Windows NSIS 安装包，一份不内置 WebView2，一份内置离线 WebView2。
- 上述实现严格基于 `0.9.1` 重做，不得直接引入远端现有 `0.9.2` 提交。
- 本机 `1.0.0` 分支为 `codex/1.0.0-personal-growth-system`，从本机独立实现的 `0.9.2` 提交 `d9ed59b` 继续；不得改从 `main` 或远端 M9.2 分支取代码。

## 开发流程

当前迭代为 `1.2.0`，分支 `codex/1.2.0-historical-unfinished-reschedule`，从经验证的 `baseline/1.1.0` 提交 `f986d28` 继续。1.1 产品与隐私边界见 `docs/V1_1_PLANNING_PRIVACY.md`，历史未完成规则见 `docs/V1_2_HISTORICAL_TASKS.md`。

1. 先读 `README.md`、`docs/HANDOFF.md`、`docs/DECISIONS.md`、`docs/CONVERSATION_HANDOFF.md` 和 `docs/BACKLOG.md`。
2. 先诊断和列出影响文件，再修改。
3. 保留用户已有改动；禁止 `git reset --hard`、强制覆盖和批量删除。
4. 每项行为变化必须同步测试与文档。
5. 提交前运行：
   - `npm run check`
   - `cargo test --locked --manifest-path src-tauri/Cargo.toml`
6. 前端安装统一使用 `npm ci`，Cargo 验证统一使用 `--locked`。
7. Windows 发行变更必须真实构建目标安装包并核对文件名与 SHA-256。
   - 用户于 2026-09-06 取消每轮完整 demo 验收要求，改为用户自行试用反馈。保留相关自动化测试和编译检查，不再反复执行或核查完整桌面 demo 门禁。
   - 如需针对性演示，使用隔离数据库和测试 Vault，禁止写入真实个人记录。
8. 使用独立分支和 PR；CI 全绿后再合并。

## 版本与产物

- 版本号必须同时更新 `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`。
- 不提交 `node_modules`、`dist`、`src-tauri/target`、真实数据库、Vault 或个人路径。
- 应提交依赖锁文件、迁移脚本、测试、发行脚本和交接文档。
- 对用户汇报时使用中文，明确列出基线提交、测试结果、安装包类型及未完成项。

## Windows 未签名包约束

- `bundle.publisher` 不是 Authenticode 签名，不能据此声称发行者可信。
- 没有证书时，保留两份 NSIS，并额外生成 no-WebView2 MSI 供企业 IT 部署。
- 每次构建必须生成 `SHA256SUMS.txt` 与 `BUILD-INFO.txt`，记录真实签名状态。
- 不得加入关闭 Defender、自动解除文件阻止或绕过公司安全策略的脚本。
- 未签名产物只能称为“经 CI 校验的未签名构建”，不能保证 SmartScreen 放行。
