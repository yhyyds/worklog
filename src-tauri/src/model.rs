use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DayTask {
    pub id: String,
    pub permanent_task_id: String,
    pub parent_id: Option<String>,
    pub display_code: String,
    pub title: String,
    pub status: String,
    pub importance: String,
    pub urgency: String,
    pub planned_start: Option<String>,
    pub planned_end: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEvent {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub occurred_at: String,
    pub title: String,
    pub detail: Option<String>,
    pub visibility: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FocusSession {
    pub id: String,
    pub task_id: String,
    pub status: String,
    pub planned_seconds: i64,
    pub remaining_seconds: i64,
    pub target_end_at: Option<String>,
    pub started_at: String,
    pub timer_mode: String,
    pub elapsed_seconds: i64,
    pub running_started_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RestSession {
    pub id: String,
    pub rest_kind: String,
    pub status: String,
    pub planned_seconds: i64,
    pub remaining_seconds: i64,
    pub target_end_at: Option<String>,
    pub started_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DayState {
    pub work_date: String,
    pub tasks: Vec<DayTask>,
    pub timeline: Vec<TimelineEvent>,
    pub focus: Option<FocusSession>,
    pub rest: Option<RestSession>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskInput {
    pub work_date: String,
    pub title: String,
    pub importance: String,
    pub urgency: String,
    pub parent_id: Option<String>,
    pub planned_start: Option<String>,
    pub planned_end: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaskInput {
    pub work_date: String,
    pub instance_id: String,
    pub title: String,
    pub planned_start: Option<String>,
    pub planned_end: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetTaskStatusInput {
    pub work_date: String,
    pub instance_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkEntryInput {
    pub work_date: String,
    pub content: String,
    pub entry_type: String,
    pub review_level: String,
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartFocusInput {
    pub work_date: String,
    pub task_id: String,
    pub planned_seconds: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusActionInput {
    pub work_date: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PauseFocusInput {
    pub work_date: String,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchFocusInput {
    pub work_date: String,
    pub task_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteFocusInput {
    pub work_date: String,
    pub reason: String,
}
