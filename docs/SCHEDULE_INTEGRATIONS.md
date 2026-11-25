# Worklog 2.0 日程集成

## 在 Worklog 内试用

1. 打开「日程集成」，选择「起草新任务」及目标日期。
2. 输入需求，例如「下午准备客户汇报，放在重要·紧急，拆为核对数据和制作材料，给出工作量和可用时段」。
3. 点「预览发送内容」，核对当天任务、固定安排和输入的需求，再输入 DeepSeek API Key，点「发送并起草任务」。密钥在关闭面板时清空。
4. 在「待确认的日程提案」核对主任务象限、子任务、时段与工作量。点「批准创建」一次性保存整组任务。

此流程使用 Worklog 内置助手，安装包内已包含所需代码。使用 DeepSeek 账号下的 API Key 和网络即可。未排期的任务可留空时段。批准时，Worklog 按手动新增的相同规则分配编号、继承象限和记入当天时间轴，主任务和子任务在同一个数据库事务中保存。

## Worklog MCP

打开「日程集成 → 外部软件接口」，选择「MCP · 提案需确认」并启动接口。把面板中的 `/mcp` 地址和 MCP 令牌配置到同一台电脑运行的 DeepSeek Harness。MCP 会话与 REST API 会话互斥，连接使用本机地址和本次会话的 Bearer 令牌。

| MCP 工具 | 能力 |
| --- | --- |
| `read_schedule` | 读取指定日期的任务、固定安排和版本摘要。任务带有当日实例 ID。 |
| `propose_reschedule` | 为当天或未来某天的一项现有任务建议同日新时段。需要带上读取所得的实例 ID 和版本摘要。 |
| `propose_create_task` | 提议在所选日期的一个象限创建主任务及子任务。提交日期、日程版本和任务组。 |

在「待确认的日程提案」查看并逐项批准。批准时 Worklog 重新核对该日期的日程。若日程已变化，拒绝旧提案并让助手重新读取。改期只更新该实例的计划时间，并追加一条改期事件。拒绝提案不写入日程。已完成、已取消、正在专注的任务不能提出改期。同一任务最多有一条待处理改期。停止 MCP 会清空外部提案，退出 Worklog 会清空本次运行的所有待确认提案。

MCP 提供普通日程任务和固定安排，随笔、工作想法、历史事件和数据库路径均不通过此连接公开。连接守住 Worklog 的操作边界，Harness 中另外启用的 shell、文件读写等工具独立受其权限控制。若 Harness 使用云端模型，模型可收到此工具返回的日程内容。

### 在 DeepSeek Harness 中启用

保存下列内容为 `worklog.cordis.yml`。它向已配置好模型的 Harness 添加一个 Worklog MCP 客户端。参考 [Harness MCP 客户端配置](https://github.com/deepseek-ai/deepseek-harness/blob/master/packages/mcp/mcp-client/README.md)。

```yaml
- insert:
    - id: mcp-worklog
      name: '@deepseek-ai/dsh-mcp-client'
      config:
        serverName: worklog
        transport: streamable-http
        url: !!js process.env.WORKLOG_MCP_URL
        headers:
          Authorization: !!js '`Bearer ${process.env.WORKLOG_MCP_TOKEN}`'
```

在文件所在目录打开 PowerShell，按提示粘贴 Worklog 面板里的地址和令牌。地址包括 `/mcp`。每次重启 Worklog 接口后重新输入地址和令牌，再重启 Harness。

```powershell
$env:WORKLOG_MCP_URL = Read-Host 'Worklog MCP 地址'
$env:WORKLOG_MCP_TOKEN = [System.Net.NetworkCredential]::new('', (Read-Host 'Worklog MCP 令牌' -AsSecureString)).Password
dsh web --patch ./worklog.cordis.yml
```

可添加到 Harness 的 Worklog 使用说明

> 先用 Worklog 读取我指定日期的任务和固定安排。起草新任务时，按用户选择的象限提交一个主任务以及至多一层子任务。子任务的工作量用轻松（小于 1 小时）、较大（1–2 小时）、极大（2–3 小时），更大的任务先拆分。存在时段冲突或缺少关键资料时，先说明并询问。提交提案时带上刚读取的日程版本。等我在 Worklog 批准，才算完成。

## 本机 API

在「日程集成」选择 REST API 的只读或写入模式并启动。Worklog 在 `127.0.0.1` 分配一个端口，面板显示含 `/v1` 的地址。接口随应用退出停止。停止并重新启用会更换端口和令牌。

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

「获取排期建议」以文本回答日程问题。应用使用 Windows 系统自带的 `curl.exe` 访问 DeepSeek。发送预览包括选定日期的任务、时段、状态、固定安排和你输入的需求。随笔、时间线原文、访问令牌和数据库文件均在本地。

## 团队集成设想

建议先用一场测试会议和一篇测试随笔联调。飞书会议纪要随其权限留在企业账号内。豆包提取行动项后，按来源标识、负责人身份和待核实字段写入团队行动项收件箱。负责人 Worklog 客户端主动拉取属于自己的事项，重复推送按来源标识合并。随笔只提供用户主动选定的 Markdown 或选区。

排期阶段由 AI 建议工作量、优先级和依赖，Worklog 根据已有安排和忙闲时段计算可用时间。同一人的专注时段互斥，等待回复可以与独立任务并行。建议先预览拟排期和冲突，再确认。提醒由程序依据已确认时间触发；需要跨设备、关机仍可送达时，可由组织批准的飞书服务承担。

这一阶段需要额外开发行动项收件箱、团队身份映射和跨机器连接；本机地址不能由云端豆包或另一台电脑直接访问。
