//! Debug-only, explicitly requested fixture. Never opens the normal app directory.
use crate::{commands, db, model::{CreateTaskInput, SetTaskStatusInput}};
use chrono::{Datelike, Duration, Local};
use rusqlite::params;
use std::{fs, path::PathBuf};

pub fn prepare() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let directory = std::env::temp_dir().join(format!("worklog-demo-{}", db::new_id()));
    fs::create_dir(&directory)?;
    let vault = directory.join("DemoVault");
    fs::create_dir(&vault)?;
    let mut connection = db::open_database(&directory.join("worklog.db"))?;
    let today = Local::now().date_naive();
    let monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
    for offset in -28..=0 {
        let date = today + Duration::days(offset);
        let text = date.to_string();
        for number in 0..3 {
            let day = commands::create_task_core(&mut connection, CreateTaskInput {
                work_date:text.clone(), title:format!("演示 · {}",["认识常见电器元件","练习识读控制回路","整理图形符号笔记"][number]),
                parent_id:None, importance:"important".into(), urgency:if number==0 {"urgent"} else {"relaxed"}.into(), planned_start:None, planned_end:None,
            })?;
            let task = day.tasks.last().unwrap();
            let instance = task.id.clone();
            if number < 2 || offset % 3 == 0 {
                commands::set_task_status_core(&mut connection, SetTaskStatusInput { work_date:text.clone(),instance_id:instance.clone(),status:"completed".into() })?;
            }
            let session = db::new_id();
            let seconds = 1200 + number as i64 * 600;
            let started = format!("{text}T0{}:00:00Z",number+1);
            let ended = format!("{text}T0{}:{:02}:00Z",number+1,seconds/60);
            connection.execute("INSERT INTO focus_sessions(id,work_date,status,primary_task_instance_id,planned_seconds,remaining_seconds,started_at_utc,ended_at_utc) VALUES(?1,?2,'completed',?3,?4,0,?5,?6)",params![session,text,instance,seconds,started,ended])?;
            connection.execute("INSERT INTO focus_segments(id,focus_session_id,task_instance_id,started_at_utc,ended_at_utc,allocated_seconds) VALUES(?1,?2,?3,?4,?5,?6)",params![db::new_id(),session,instance,started,ended,seconds])?;
        }
    }
    let now = db::now_iso();
    let start = (today-Duration::days(28)).to_string();
    for (id,title) in [("demo-shower","到家后洗澡"),("demo-sleep","22:00 前睡觉")] {
        connection.execute("INSERT INTO habits(id,title,start_date,created_at_utc,updated_at_utc) VALUES(?1,?2,?3,?4,?4)",params![id,title,start,now])?;
    }
    connection.execute("INSERT INTO habit_dependencies(habit_id,prerequisite_habit_id) VALUES('demo-sleep','demo-shower')",[])?;
    for offset in -28..-1 {
        let date = (today+Duration::days(offset)).to_string();
        let effective = offset % 4 != 0;
        connection.execute("INSERT INTO habit_reviews VALUES(?1,?2)",params![date,now])?;
        for id in ["demo-shower","demo-sleep"] {
            let blocked = if !effective && id=="demo-sleep" { "[\"demo-shower\"]" } else { "[]" };
            connection.execute("INSERT INTO habit_occurrences VALUES(?1,?2,?3,?4,?5,?6)",params![id,date,id=="demo-sleep" || effective,effective,blocked,now])?;
        }
    }
    connection.execute("INSERT INTO long_term_goals(id,title,cycle_days,start_date,created_at_utc,updated_at_utc) VALUES('demo-goal','演示 · 一个月读懂电气原理图',30,?1,?2,?2)",params![monday.to_string(),now])?;
    connection.execute("INSERT INTO goal_phases(id,goal_id,title,start_date,end_date,brainstorm_md,created_at_utc,updated_at_utc) VALUES('demo-phase','demo-goal','第一周 · 元件与符号',?1,?2,'是不是先认识常用元件，再试着读一段回路？每天给自己留一个小练习。',?3,?3)",params![monday.to_string(),(monday+Duration::days(6)).to_string(),now])?;
    connection.execute("INSERT INTO goal_actions(id,phase_id,title,action_kind,required,target_count,completed_count,created_at_utc,updated_at_utc) VALUES('demo-required','demo-phase','识读练习','repeating',1,5,5,?1,?1),('demo-optional','demo-phase','拓展阅读','one_off',0,1,1,?1,?1)",[&now])?;
    connection.execute("INSERT INTO tasks(id,title,created_at_utc,updated_at_utc) VALUES('demo-inbox','演示 · 有空时整理资料库',?1,?1)",[&now])?;
    connection.execute("INSERT INTO task_inbox(task_id,group_id,importance,urgency,entered_at_utc) VALUES('demo-inbox','demo-inbox','secondary','relaxed',?1)",[&now])?;
    let settings = serde_json::json!({"vaultPath":vault.to_string_lossy(),"dailyRoot":""});
    connection.execute("INSERT INTO app_settings VALUES('obsidian',?1,?2)",params![settings.to_string(),now])?;
    fs::write(directory.join("DEMO-ONLY.txt"),"隔离演示数据，可保留用于验收。不是用户正式数据库。\n")?;
    Ok(directory)
}
