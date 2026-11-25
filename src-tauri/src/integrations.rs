use crate::{commands, db, model::*, Database};
use chrono::{Local, NaiveDate};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, State};

#[path = "schedule_mcp.rs"]
mod schedule_mcp;
use schedule_mcp::{Proposal, Proposals};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApiSession {
    url: String,
    read_token: String,
    write_token: Option<String>,
    mcp_mode: bool,
}

struct RunningApi {
    session: ApiSession,
    stopped: Arc<AtomicBool>,
    proposals: Arc<Proposals>,
}

impl Drop for RunningApi {
    fn drop(&mut self) { self.stopped.store(true, Ordering::Release); }
}

#[derive(Default)]
pub(crate) struct LocalApi(Mutex<Option<RunningApi>>);

#[tauri::command]
pub(crate) fn local_api_status(api: State<'_, LocalApi>) -> Option<ApiSession> {
    api.0.lock().ok()?.as_ref().map(|running| running.session.clone())
}

#[tauri::command]
pub(crate) fn stop_local_api(api: State<'_, LocalApi>) -> Result<(), String> {
    api.0.lock().map_err(|_| "接口状态不可用")?.take();
    Ok(())
}

#[tauri::command]
pub(crate) fn start_local_api(app: AppHandle, api: State<'_, LocalApi>, allow_write: bool, mcp_mode: bool) -> Result<ApiSession, String> {
    if mcp_mode && allow_write { return Err("MCP 模式的提案须经 Worklog 确认".into()); }
    let mut current = api.0.lock().map_err(|_| "接口状态不可用")?;
    if current.is_some() { return Err("请先关闭当前接口，再重新启用".into()); }
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|error| format!("无法启动本机接口：{error}"))?;
    listener.set_nonblocking(true).map_err(|error| error.to_string())?;
    let host = listener.local_addr().map_err(|error| error.to_string())?.to_string();
    let token = || format!("{}{}", db::new_id(), db::new_id());
    let path = if mcp_mode { "mcp" } else { "v1" };
    let session = ApiSession { url: format!("http://{host}/{path}"), read_token: token(), write_token: allow_write.then(token), mcp_mode };
    let stopped = Arc::new(AtomicBool::new(false));
    let worker_stop = stopped.clone();
    let worker_session = session.clone();
    let proposals = Arc::new(Proposals::default());
    let worker_proposals = proposals.clone();
    thread::Builder::new().name("worklog-local-api".into()).spawn(move || {
        while !worker_stop.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let database = app.state::<Database>();
                    let (status, value, changed) = handle(&mut stream, &host, &worker_session, &worker_stop, &database, &worker_proposals);
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                    let _ = respond(&mut stream, status, &value);
                    if changed { let _ = app.emit("worklog-schedule-changed", ()); }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(40)),
                Err(_) => break,
            }
        }
    }).map_err(|error| error.to_string())?;
    *current = Some(RunningApi { session: session.clone(), stopped, proposals });
    Ok(session)
}

#[tauri::command]
pub(crate) fn list_schedule_proposals(api: State<'_, LocalApi>) -> Result<Vec<Proposal>, String> {
    let current = api.0.lock().map_err(|_| "接口状态不可用")?;
    match current.as_ref().filter(|running| running.session.mcp_mode) {
        Some(running) => running.proposals.list(),
        None => Ok(Vec::new()),
    }
}

#[tauri::command]
pub(crate) fn decide_schedule_proposal(
    app: AppHandle, api: State<'_, LocalApi>, database: State<'_, Database>, id: String, approve: bool,
) -> Result<(), String> {
    {
        let current = api.0.lock().map_err(|_| "接口状态不可用")?;
        let running = current.as_ref().filter(|running| running.session.mcp_mode).ok_or("MCP 已停止")?;
        let mut connection = database.0.lock().map_err(|_| "数据库繁忙")?;
        running.proposals.decide(&mut connection, &id, approve)?;
    }
    if approve { let _ = app.emit("worklog-schedule-changed", ()); }
    Ok(())
}

const MAX_BODY: usize = 32 * 1024;
type ApiError = (u16, String);
type ApiResult<T> = Result<T, ApiError>;

fn bad(message: &str) -> ApiError { (400, message.into()) }
fn internal(_: String) -> ApiError { (500, "无法读取日程".into()) }

fn date(text: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").ok().filter(|value| value.to_string() == text)
        .ok_or_else(|| "日期必须为 YYYY-MM-DD".into())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScheduleSnapshot {
    work_date: String,
    tasks: Vec<DayTask>,
    schedules: Vec<DailySchedule>,
    revision: String,
}

fn snapshot(day: DayState) -> ScheduleSnapshot {
    let revision = hex::encode(Sha256::digest(
        serde_json::to_vec(&(&day.work_date, &day.tasks, &day.schedules)).expect("schedule DTO is serializable"),
    ));
    ScheduleSnapshot { work_date: day.work_date, tasks: day.tasks, schedules: day.schedules, revision }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MutationRequest {
    expected_revision: String,
    operation: Mutation,
}

#[derive(Deserialize)]
#[serde(tag = "action", content = "input", rename_all = "camelCase")]
enum Mutation {
    CreateTask(CreateTaskInput),
    UpdateTask(UpdateTaskInput),
    SetTaskStatus(SetTaskStatusInput),
    SaveSchedule(SaveDailyScheduleInput),
    CancelSchedule(CancelDailyScheduleInput),
}

fn mutate(connection: &mut Connection, work_date: &str, request: MutationRequest) -> ApiResult<ScheduleSnapshot> {
    let day = db::read_day(connection, work_date).map_err(internal)?;
    if snapshot(day.clone()).revision != request.expected_revision {
        return Err((409, "日程已有变化，请重新读取".into()));
    }
    let visible = |id: &str| day.tasks.iter().any(|task| task.id == id);
    let same_date = |input_date: &str| if input_date == work_date { Ok(()) } else { Err(bad("请求日期与路径不一致")) };
    let result = match request.operation {
        Mutation::CreateTask(input) => {
            same_date(&input.work_date)?;
            if input.parent_id.as_deref().is_some_and(|id| !visible(id)) { return Err(bad("父任务不在日程中")); }
            commands::create_task_core(connection, input)
        }
        Mutation::UpdateTask(input) => {
            same_date(&input.work_date)?;
            if !visible(&input.instance_id) { return Err(bad("任务不在日程中")); }
            commands::update_task_core(connection, input)
        }
        Mutation::SetTaskStatus(input) => {
            same_date(&input.work_date)?;
            if !visible(&input.instance_id) { return Err(bad("任务不在日程中")); }
            if day.focus.as_ref().is_some_and(|focus| focus.task_id == input.instance_id) {
                return Err(bad("请先结束此任务的专注"));
            }
            commands::set_task_status_core(connection, input)
        }
        Mutation::SaveSchedule(input) => {
            same_date(&input.work_date)?;
            commands::save_daily_schedule_core(connection, input)
        }
        Mutation::CancelSchedule(input) => {
            same_date(&input.work_date)?;
            commands::cancel_daily_schedule_core(connection, input)
        }
    };
    result.map(snapshot).map_err(|error| (422, error))
}

fn read_chunk(stream: &mut TcpStream, bytes: &mut [u8], deadline: Instant) -> ApiResult<usize> {
    let timeout = deadline.saturating_duration_since(Instant::now());
    if timeout.is_zero() { return Err((408, "请求超时".into())); }
    stream.set_read_timeout(Some(timeout)).map_err(|_| bad("无法读取请求"))?;
    stream.read(bytes).map_err(|_| (408, "请求未读完".into()))
}

fn request(
    stream: &mut TcpStream, host: &str, session: &ApiSession, stopped: &AtomicBool,
    database: &Database, proposals: &Proposals,
) -> ApiResult<(u16, Value, bool)> {
    let deadline = Instant::now() + Duration::from_secs(4);
    let mut buffer = Vec::new();
    let mut chunk = [0; 2048];
    let header_end = loop {
        if let Some(index) = buffer.windows(4).position(|part| part == b"\r\n\r\n") { break index + 4; }
        if buffer.len() >= 8192 { return Err(bad("请求头过长")); }
        let count = read_chunk(stream, &mut chunk, deadline)?;
        if count == 0 { return Err(bad("请求不完整")); }
        buffer.extend_from_slice(&chunk[..count]);
    };
    if header_end > 8192 { return Err(bad("请求头过长")); }
    let mut headers = [httparse::EMPTY_HEADER; 32];
    let mut parsed = httparse::Request::new(&mut headers);
    if !matches!(parsed.parse(&buffer[..header_end]), Ok(httparse::Status::Complete(_))) {
        return Err(bad("无效 HTTP 请求"));
    }
    let header = |name: &str| -> ApiResult<Option<&[u8]>> {
        let mut matches = parsed.headers.iter().filter(|field| field.name.eq_ignore_ascii_case(name));
        let value = matches.next().map(|field| field.value);
        if matches.next().is_some() { return Err(bad("重复请求头")); }
        Ok(value)
    };
    if header("host")? != Some(host.as_bytes()) || header("origin")?.is_some() || header("transfer-encoding")?.is_some() {
        return Err((403, "仅接受本机程序发出的直接请求".into()));
    }
    let write = match parsed.method {
        Some("GET") => false,
        Some("POST") => true,
        _ => return Err((405, "仅支持 GET 或 POST".into())),
    };
    let authorization = header("authorization")?.and_then(|value| value.strip_prefix(b"Bearer "));
    let accepts = |token: &str| {
        let Some(value) = authorization else { return false; };
        value.len() == token.len() && value.iter().copied().zip(token.bytes()).fold(0, |bits, (a, b)| bits | (a ^ b)) == 0
    };
    let path = parsed.path.ok_or(bad("请求路径缺失"))?;
    let work_date = if session.mcp_mode {
        if !accepts(&session.read_token) { return Err((401, "令牌无效或权限不足".into())); }
        if path != "/mcp" { return Err((404, "未找到接口".into())); }
        if !write { return Err((405, "MCP 端点仅接受 POST".into())); }
        if header("mcp-protocol-version")?.is_some_and(|value| !schedule_mcp::supports_version(value)) {
            return Err(bad("不支持的 MCP 协议版本"));
        }
        None
    } else {
        let can_write = session.write_token.as_deref().is_some_and(accepts);
        if !can_write && (write || !accepts(&session.read_token)) { return Err((401, "令牌无效或权限不足".into())); }
        let work_date = path.strip_prefix("/v1/days/").ok_or((404, "未找到接口".into()))?;
        Some((work_date.to_string(), date(work_date).map_err(|error| (400, error))?))
    };
    let length = header("content-length")?.map(|raw| {
        std::str::from_utf8(raw).ok().filter(|s| s.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|s| s.parse::<usize>().ok()).ok_or(bad("无效请求长度"))
    }).transpose()?.unwrap_or(0);
    if length > MAX_BODY { return Err((413, "请求内容过大".into())); }
    let json_body = header("content-type")?.and_then(|value| std::str::from_utf8(value).ok())
        .and_then(|value| value.split(';').next()).is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"));
    if write && !json_body {
        return Err((415, "请发送 application/json".into()));
    }
    if !write && length != 0 { return Err(bad("GET 不接受请求体")); }
    while buffer.len() < header_end + length {
        let remaining = (header_end + length - buffer.len()).min(chunk.len());
        let count = read_chunk(stream, &mut chunk[..remaining], deadline)?;
        if count == 0 { return Err(bad("请求内容不完整")); }
        buffer.extend_from_slice(&chunk[..count]);
    }
    let body = &buffer[header_end..header_end + length];
    let mut connection = database.0.lock().map_err(|_| (500, "数据库繁忙".to_string()))?;
    if stopped.load(Ordering::Acquire) { return Err((403, "接口已停用".into())); }
    let Some((work_date, parsed_date)) = work_date else {
        let (code, value) = schedule_mcp::rpc(&connection, proposals, body);
        return Ok((code, value, false));
    };
    if write {
        if parsed_date < Local::now().date_naive() { return Err((403, "历史日程只能读取".into())); }
        let body = serde_json::from_slice(body).map_err(|error| (400, format!("无效的操作数据：{error}")))?;
        return mutate(&mut connection, &work_date, body).map(|value| (200, json!(value), true));
    }
    let day = db::read_day(&connection, &work_date).map_err(internal)?;
    Ok((200, json!(snapshot(day)), false))
}

fn handle(
    stream: &mut TcpStream, host: &str, session: &ApiSession, stopped: &AtomicBool,
    database: &Database, proposals: &Proposals,
) -> (u16, Value, bool) {
    match request(stream, host, session, stopped, database, proposals) {
        Ok(reply) => reply,
        Err((status, error)) => (status, json!({ "error": error }), false),
    }
}

fn respond(stream: &mut TcpStream, code: u16, value: &Value) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK", 202 => "Accepted", 400 => "Bad Request", 401 => "Unauthorized", 403 => "Forbidden",
        404 => "Not Found", 405 => "Method Not Allowed", 408 => "Request Timeout", 409 => "Conflict",
        413 => "Content Too Large", 415 => "Unsupported Media Type", 422 => "Unprocessable Content",
        _ => "Internal Server Error",
    };
    let body = if code == 202 { String::new() } else { value.to_string() };
    let content_type = if body.is_empty() { "" } else { "Content-Type: application/json; charset=utf-8\r\n" };
    write!(stream, "HTTP/1.1 {code} {reason}\r\n{content_type}Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}", body.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use schedule_mcp::CREATE_TASK_TOOL;

    fn database() -> Database {
        let connection = Connection::open_in_memory().unwrap();
        db::initialize(&connection).unwrap();
        Database(Mutex::new(connection))
    }

    fn call_session(
        database: &Database, session: &ApiSession, proposals: &Proposals, make_request: impl FnOnce(&str) -> String,
    ) -> (u16, Value, bool) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let request = make_request(&addr.to_string());
        let mut client = TcpStream::connect(addr).unwrap();
        client.write_all(request.as_bytes()).unwrap();
        let (mut stream, _) = listener.accept().unwrap();
        handle(&mut stream, &addr.to_string(), session, &AtomicBool::new(false), database, proposals)
    }

    fn call(database: &Database, make_request: impl FnOnce(&str) -> String) -> (u16, Value, bool) {
        let session = ApiSession { url: String::new(), read_token: "reader".into(), write_token: Some("writer".into()), mcp_mode: false };
        call_session(database, &session, &Proposals::default(), make_request)
    }

    #[test]
    fn local_api_limits_access_and_rejects_stale_writes() {
        let database = database();
        let day = Local::now().date_naive().to_string();
        let get = |host: &str, token: &str, extra: &str| format!("GET /v1/days/{day} HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer {token}\r\n{extra}\r\n");
        assert_eq!(call(&database, |host| get(host, "unknown", "")).0, 401);
        assert_eq!(call(&database, |_| get("example.com", "reader", "")).0, 403);
        assert_eq!(call(&database, |host| get(host, "reader", "Origin: https://example.com\r\n")).0, 403);
        let (status, read, changed) = call(&database, |host| get(host, "reader", ""));
        assert_eq!(status, 200);
        assert!(!changed);
        assert!(read.get("timeline").is_none());
        let body = json!({
            "expectedRevision": read["revision"],
            "operation": { "action": "saveSchedule", "input": {
                "workDate": day, "title": "设计评审", "plannedStart": "10:00", "plannedEnd": "11:00"
            }}
        }).to_string();
        let post = |host: &str, token: &str| format!("POST /v1/days/{day} HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\n\r\n{body}", body.len());
        assert_eq!(call(&database, |host| post(host, "reader")).0, 401);
        let (status, result, changed) = call(&database, |host| post(host, "writer"));
        assert_eq!(status, 200, "{result}");
        assert!(changed);
        assert_eq!(result["schedules"][0]["title"], "设计评审");
        assert_eq!(call(&database, |host| post(host, "writer")).0, 409);
        let connection = database.0.lock().unwrap();
        let saved = db::read_day(&connection, &day).unwrap();
        assert_eq!(saved.schedules.len(), 1);
        assert_eq!(saved.timeline[0].event_type, "schedule.created");
    }

    #[test]
    fn mcp_transport_exposes_only_the_allowed_tools() {
        let database = database();
        let proposals = Proposals::default();
        let session = ApiSession { url: String::new(), read_token: "mcp-token".into(), write_token: None, mcp_mode: true };
        let post = |path: &str, token: &str, headers: &str, body: Value| {
            let body = body.to_string();
            call_session(&database, &session, &proposals, |host| format!(
                "POST {path} HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{headers}\r\n{body}",
                body.len(),
            ))
        };
        let initialize = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-11-25"}});
        assert_eq!(post("/mcp", "wrong", "", initialize.clone()).0, 401);
        assert_eq!(post("/mcp", "mcp-token", "Origin: https://example.com\r\n", initialize.clone()).0, 403);
        assert_eq!(post("/v1/days/2026-11-25", "mcp-token", "", initialize.clone()).0, 404);
        assert_eq!(post("/mcp", "mcp-token", "MCP-Protocol-Version: invalid\r\n", initialize.clone()).0, 400);
        let (status, response, changed) = post("/mcp", "mcp-token", "", initialize);
        assert_eq!(status, 200);
        assert!(!changed);
        assert_eq!(response["result"]["protocolVersion"], "2025-11-25");
        let (_, response, _) = post("/mcp", "mcp-token", "MCP-Protocol-Version: 2025-11-25\r\n",
            json!({"jsonrpc": "2.0", "id": "tools", "method": "tools/list"}));
        let names: Vec<_> = response["result"]["tools"].as_array().unwrap().iter()
            .map(|tool| tool["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["read_schedule", "propose_reschedule", CREATE_TASK_TOOL]);
        let (_, response, changed) = post("/mcp", "mcp-token", "",
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "approve_reschedule"}}));
        assert_eq!(response["result"]["isError"], true);
        assert!(!changed);
        let (status, response, _) = post("/mcp", "mcp-token", "",
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        assert_eq!((status, response), (202, Value::Null));
        let (status, _, _) = call_session(&database, &session, &proposals, |host| format!(
            "GET /mcp HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer mcp-token\r\n\r\n",
        ));
        assert_eq!(status, 405);

        let work_date = Local::now().date_naive().to_string();
        let (_, response, _) = post("/mcp", "mcp-token", "", json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {"name": "read_schedule", "arguments": {"workDate": work_date}}
        }));
        let read: Value = serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        let (status, response, changed) = post("/mcp", "mcp-token", "", json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {
                "name": CREATE_TASK_TOOL, "arguments": {
                    "workDate": work_date, "expectedRevision": read["revision"],
                    "plan": {"title": "准备汇报", "importance": "important", "urgency": "urgent", "subtasks": []}
                }
            }
        }));
        let draft: Value = serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(status, 200);
        assert!(!changed);
        assert_eq!(draft["status"], "pending_approval");
        assert_eq!(proposals.list().unwrap().len(), 1);
        let connection = database.0.lock().unwrap();
        assert!(db::read_day(&connection, &work_date).unwrap().tasks.is_empty());
    }
}
