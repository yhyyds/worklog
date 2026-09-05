# 本机 Codex 首次接管提示词

将以下内容完整粘贴给在仓库根目录运行的 Codex。

```text
你现在接管的是私有仓库 yhyyds/worklog。请把本次工作视为一次严格的无损本地接管，而不是普通功能修改。

一、硬性基线

1. 本轮唯一允许的代码基线是 Worklog 0.9.1。
2. 基线提交必须是 7d2fdfc203ff55358abfd7b9dfdebb5140a27fb3。
3. 当前工作分支应为 codex/0.9.1-local-handoff，或从该分支新建的功能分支。
4. main 上存在更晚的 0.9.2，但不得 merge、rebase 或 cherry-pick 该实现。它只能用于历史参考。
5. 不允许执行 git reset --hard、强制覆盖、清理真实数据库或删除 Obsidian 文件。

二、先读取上下文，不要立即改代码

请依次完整读取：
- AGENTS.md
- README.md
- CHANGELOG.md
- docs/HANDOFF.md
- docs/DECISIONS.md
- docs/CONVERSATION_HANDOFF.md
- docs/BACKLOG.md
- docs/ARCHITECTURE.md
- docs/PRODUCT_BASELINE.md
- docs/M3_OBSIDIAN_SYNC.md
- docs/M6_FOCUS_LIFECYCLE.md
- docs/M7_WINDOWS_RELEASE.md
- docs/M9_USAGE_REFINEMENTS.md
- docs/LOCAL_MIGRATION.md

随后执行只读检查：
- git status --short
- git branch --show-current
- git rev-parse HEAD
- git merge-base --is-ancestor 7d2fdfc203ff55358abfd7b9dfdebb5140a27fb3 HEAD
- 检查 package.json、src-tauri/Cargo.toml、src-tauri/tauri.conf.json 三处版本。
- 检查 package-lock.json 与 src-tauri/Cargo.lock 是否存在。
- 枚举可用的 npm 脚本、Rust 测试和 Windows 打包工作流。

三、先向我报告

在修改任何文件前，输出一份简洁但完整的接管报告，必须包含：
1. 当前分支、HEAD、0.9.1 祖先校验结果。
2. 三处版本号是否一致。
3. 当前产品功能和主要架构。
4. SQLite 与 Obsidian 的数据安全红线。
5. 当前 0.9.1 日记格式的真实行为。
6. 下一轮待实现需求。
7. 依赖锁定是否完整。
8. 计划执行的测试和打包命令。
如发现任何一项与交接文档不一致，立即停止并询问，不要自行选择其他基线。

四、本机依赖与基线验证

如果锁文件不存在：
1. 使用当前 package.json 和 Cargo.toml 生成 package-lock.json 与 src-tauri/Cargo.lock。
2. 不升级 package.json 或 Cargo.toml 中声明的依赖范围。
3. 记录 node --version、npm --version、rustc --version 和 cargo --version。
4. 运行 npm run check。
5. 运行 cargo test --locked --manifest-path src-tauri/Cargo.toml。
6. 两套验证全部通过后，单独提交锁文件，提交信息为：chore: lock 0.9.1 dependencies for local handoff。

五、下一轮功能目标

必须从 0.9.1 重新实现以下需求：

A. Obsidian 原生折叠
- 删除每日日记中的所有 <details> 与 <summary> 输出。
- 每轮专注写成一条父级 Markdown 列表记录，格式语义为：第N轮任务，专注时段：HH:MM-HH:MM，任务记录：。
- 该轮时间轴事件紧接在父级行之后。
- 每条子事件前缩进一个制表位。
- 父级行与第一条子事件之间不得有空白行。
- 不使用 HTML、JavaScript 或 Obsidian 私有插件语法。
- 保留非专注时段记录与现有降噪规则。
- 增加精确 Markdown 回归测试，覆盖多轮专注、暂停原因、任务切换和空事件轮次。

B. 两份 Windows 安装包
- 第一份不内置 WebView2，体积较小。
- 第二份内置 WebView2 离线运行时，可离线安装。
- 文件名必须明确包含 no-webview2 与 with-webview2。
- 生成 SHA256SUMS.txt。
- CI 和 Release 都必须构建、校验并上传两份安装包。
- README 和发行说明要明确解释适用场景。

六、实现约束

- SQLite 始终是结构化事实源。
- 不改变任务永久 ID、每日编号和事件不可变规则。
- 不覆盖受管理区块之外的 Obsidian 人工内容。
- 不让测试访问真实 worklog.db 或真实 Vault。
- 写文件继续使用备份和原子替换。
- 所有行为改变必须配套自动化测试。
- 版本变化必须同步 package.json、Cargo.toml 和 tauri.conf.json。
- 不提交 node_modules、dist、src-tauri/target、数据库、Vault 或个人绝对路径。

七、工作方式

1. 接管报告获得确认后，再给出分文件实施计划。
2. 在独立功能分支实施。
3. 先完成代码和测试，再运行完整验证。
4. Windows 打包必须真实执行，不能只检查配置。
5. 最终报告列出：改动文件、测试数量、两份安装包文件名/大小/SHA-256、提交 SHA、PR 链接和剩余风险。
6. CI 未全绿前不要合并。

现在只完成上下文读取、只读校验和接管报告，不要修改文件。
```