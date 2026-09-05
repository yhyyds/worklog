# Worklog 本地迁移与后续待办

## P0：本机接管门禁

- [ ] 将仓库完整镜像为 bare mirror，并生成 `--all` Git Bundle。
- [ ] 导出或归档 GitHub PR、Issue、Release、Actions 产物和原始 ChatGPT 会话。
- [ ] 从托盘彻底退出 Worklog。
- [ ] 备份实际数据库目录、`storage-location.json` 所在目录和完整 Obsidian Vault。
- [ ] 为备份生成 SHA-256 文件清单，并在另一存储介质保留副本。
- [ ] 在本机签出 `codex/0.9.1-local-handoff`，确认祖先包含 `7d2fdfc`。
- [ ] 生成并提交 `package-lock.json`、`src-tauri/Cargo.lock`。
- [ ] 记录 `node --version`、`rustc --version`、`cargo --version`。
- [ ] 运行前端、Rust、桌面启动和 Windows 安装包基线验证。

## P1：基于 0.9.1 的下一轮功能

- [ ] 删除 `src-tauri/src/obsidian.rs` 日记渲染中的 `<details>` 与 `<summary>`。
- [ ] 每轮专注输出一条父级列表记录。
- [ ] 每个时间轴事件紧随父行，并缩进一个制表位。
- [ ] 父行与首个子事件之间不插入空白行。
- [ ] 添加精确字符串回归测试，覆盖多轮专注、任务切换、暂停原因和非专注记录。
- [ ] 新增不内置 WebView2 的 Tauri 配置。
- [ ] 新增脚本一次构建两份 NSIS 安装包。
- [ ] 为两份安装包使用明确文件名并生成 SHA-256。
- [ ] 更新 README、CHANGELOG 和发行说明。
- [ ] Windows CI 真实构建并上传两份安装包。

## P2：后续发行完善

- [ ] 配置 Windows 商业代码签名证书，降低 SmartScreen 提示。
- [ ] 为依赖和工具链建立定期升级流程，不在功能迭代中隐式升级。
- [ ] 建立数据库备份恢复演练与版本升级回滚说明。

## 下一轮完成定义

- 基线明确来自 `0.9.1`，不包含现有 `0.9.2` 提交。
- 前端和 Rust 测试全部通过。
- 在真实 Windows runner 上成功产生两份安装包。
- Markdown 文件中不再出现 `<details>` 或 `<summary>`。
- Obsidian 可通过嵌套列表折叠时间轴。
- 测试未写入或覆盖真实数据库与真实 Vault。
- PR 中说明产物文件名、大小、SHA-256 和 WebView2 差异。