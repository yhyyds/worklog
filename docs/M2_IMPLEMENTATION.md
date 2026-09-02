# M2 SQLite 核心接线

## 已完成

- React 通过 WorklogGateway 调用应用用例，不再直接依赖具体存储。
- Tauri 桌面模式调用 Rust 命令并以 SQLite 作为事实源。
- 浏览器模式保留 localStorage 适配器，用于无需 Rust 的快速界面开发。
- 新建/完成任务、工作想法、专注开始/暂停/继续/切换/结束均返回新的 DayState 查询模型。
- 每个桌面写操作在一个事务中同时更新状态并追加不可变事件。
- 专注使用目标结束时间计算剩余时间，支持进程休眠后的恢复。

## IPC 命令

- get_day_snapshot
- create_task
- set_task_status
- add_work_entry
- start_focus
- pause_focus
- resume_focus
- switch_focus
- complete_focus

## 数据一致性

SQLite 唯一活动标记确保同时只能存在一个活动番茄钟。事件表触发器拒绝 UPDATE 和 DELETE。父子任务编号在事务内查询并生成，三级任务在写入前被拒绝。

## 自动测试

Rust 测试覆盖：状态与事件原子提交、子任务编号和深度、事件不可变性、单活动专注约束。前端继续覆盖显示编号与目标时间恢复。
