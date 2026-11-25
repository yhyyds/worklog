# Worklog 2.0 日程集成

## 本机 API

在「日程集成」启动接口。Worklog 在 `127.0.0.1` 分配一个端口，面板显示含 `/v1` 的地址。接口随应用退出停止。停止并重新启用会更换端口和令牌。

| 方法与路径（相对于面板地址） | 授权 | 返回值 |
| --- | --- | --- |
| `GET /days/YYYY-MM-DD` | 只读或写入令牌 | `workDate`、`tasks`、`schedules`、`revision` |
| `POST /days/YYYY-MM-DD` | 写入令牌 | 操作后的同一格式快照 |

使用 `Authorization: Bearer <令牌>`。写入发送 `Content-Type: application/json` 和请求长度，且只接受今天及未来日期。每个请求执行一项操作，读取后把返回的 `revision` 作为 `expectedRevision` 提交。遇到 `409` 先重新读取，再根据最新数据决定是否重试。日期使用电脑所在时区。示例为新增固定安排，请替换版本摘要和日期。

```json
{
  "expectedRevision": "<GET 返回的 revision>",
  "operation": {
    "action": "saveSchedule",
    "input": {
      "workDate": "2026-11-25",
      "scheduleId": null,
      "title": "选型评审",
      "plannedStart": "14:00",
      "plannedEnd": "14:30"
    }
  }
}
```

| action | input 字段（每项均包含 workDate） |
| --- | --- |
| `createTask` | `title`、`importance`、`urgency`；可选 `parentId`、`plannedStart`、`plannedEnd`、`workload` |
| `updateTask` | `instanceId`、`title`、计划起止、重要性、紧急性、工作量，未设置的字段传 `null` |
| `setTaskStatus` | `instanceId`、`status` |
| `saveSchedule` | `scheduleId`（新增传 `null`）、`title`、`plannedStart`、`plannedEnd` |
| `cancelSchedule` | `scheduleId` |

时间使用 `HH:MM`，开始早于结束。重要性取 `important` 或 `secondary`，紧急性取 `urgent` 或 `relaxed`。子任务的工作量可为 `light`、`large`、`very_large`。任务状态取 `not_started`、`in_progress`、`waiting`、`blocked`、`completed`、`deferred`、`cancelled`。更新子任务时重要性和紧急性传 `null`，更新主任务时工作量传 `null`。编号用于显示，写入传快照中对应的 `id`。

## 应用内 DeepSeek

选日期并填写问题，先查看发送预览，再输入密钥并发送。应用使用 Windows 系统自带的 `curl.exe` 访问 DeepSeek。日程预览包括任务、计划时间、状态与固定安排；随笔、时间线原文、访问令牌和数据库文件不在日程上下文中。密钥保留至面板关闭。建议以文本展示，由你决定是否调整计划。

## 团队集成设想

建议先用一场测试会议和一篇测试随笔联调。飞书会议纪要随其权限留在企业账号内。豆包提取行动项后，按来源标识、负责人身份和待核实字段写入团队行动项收件箱。负责人 Worklog 客户端主动拉取属于自己的事项，重复推送按来源标识合并。随笔只提供用户主动选定的 Markdown 或选区。

排期阶段由 AI 建议工作量、优先级和依赖，Worklog 根据已有安排和忙闲时段计算可用时间。同一人的专注时段互斥，等待回复可以与独立任务并行。建议先预览拟排期和冲突，再确认。提醒由程序依据已确认时间触发；需要跨设备、关机仍可送达时，可由组织批准的飞书服务承担。

这一阶段需要额外开发行动项收件箱、团队身份映射和跨机器连接；本机地址不能由云端豆包或另一台电脑直接访问。
