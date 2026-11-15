use crate::db::Db;
use crate::models::FinishedTask;
use crate::repo;
use crate::store::{add_local_days, local_day_start, today_start};
use chrono::{Datelike, Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightsDay {
    pub date: f64,
    pub planned: i64,
    pub completed: i64,
    pub focus_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightsSummary {
    pub days: Vec<InsightsDay>,
    pub completed: i64,
    pub planned: i64,
    pub focus_seconds: i64,
    pub active_days: i64,
    pub previous_completed: i64,
    pub headline: String,
    pub sentence: String,
}

pub fn format_duration(seconds: i64) -> String {
    let minutes = (seconds + 30) / 60;
    let h = minutes / 60;
    let m = minutes % 60;
    if h > 0 {
        if m > 0 {
            format!("{h}h {m}m")
        } else {
            format!("{h}h")
        }
    } else {
        format!("{m}m")
    }
}

pub fn compute_insights(db: &Db, week_offset: i64) -> InsightsSummary {
    let offset = week_offset.min(0);
    let today = today_start();
    let end = add_local_days(today, offset * 7);
    let start = add_local_days(end, -6);

    let mut days = Vec::with_capacity(7);
    for i in 0..7 {
        let d = add_local_days(start, i);
        let next = add_local_days(d, 1);
        days.push(InsightsDay {
            date: d,
            planned: repo::session_planned_count(db, d, next),
            completed: repo::session_completed_count(db, d, next),
            focus_seconds: repo::session_focus_seconds(db, d, next),
        });
    }

    let prev_start = add_local_days(start, -7);
    let completed: i64 = days.iter().map(|d| d.completed).sum();
    let planned: i64 = days.iter().map(|d| d.planned).sum();
    let focus_seconds: i64 = days.iter().map(|d| d.focus_seconds).sum();
    let active_days = days
        .iter()
        .filter(|d| d.completed > 0 || d.focus_seconds > 0)
        .count() as i64;
    let previous_completed = repo::session_completed_count(db, prev_start, start);

    let headline = if completed == 0 && focus_seconds == 0 {
        "A fresh start".to_string()
    } else if completed > previous_completed {
        "Momentum is building".to_string()
    } else if completed == previous_completed {
        "Holding steady".to_string()
    } else {
        "A quieter stretch".to_string()
    };

    let noun = if completed == 1 { "task" } else { "tasks" };
    let mut sentence = format!("{completed} {noun} done");
    if previous_completed > 0 || completed > 0 {
        if completed > previous_completed {
            sentence.push_str(&format!(", up from {previous_completed} the week before"));
        } else if completed < previous_completed {
            sentence.push_str(&format!(", down from {previous_completed} the week before"));
        } else {
            sentence.push_str(", the same as the week before");
        }
    }
    if focus_seconds > 0 {
        sentence.push_str(&format!(", with {} focused", format_duration(focus_seconds)));
    }
    sentence.push('.');

    InsightsSummary {
        days,
        completed,
        planned,
        focus_seconds,
        active_days,
        previous_completed,
        headline,
        sentence,
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySection {
    pub day: f64,
    pub title: String,
    pub items: Vec<FinishedTask>,
    pub focus_seconds: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub sections: Vec<HistorySection>,
    pub total: i64,
    pub can_load_more: bool,
}

pub fn format_history_day_title(day_ts: f64) -> String {
    let today = today_start();
    let yesterday = add_local_days(today, -1);
    if (day_ts - today).abs() < 1.0 {
        return "Today".into();
    }
    if (day_ts - yesterday).abs() < 1.0 {
        return "Yesterday".into();
    }
    let now_year = Local::now().year();
    let dt = Local
        .timestamp_opt(day_ts.floor() as i64, 0)
        .single()
        .unwrap_or_else(Local::now);
    if dt.year() == now_year {
        dt.format("%A, %-d %b").to_string()
    } else {
        dt.format("%a, %-d %b %Y").to_string()
    }
}

pub fn load_history(db: &Db, search: &str, page_size: i64) -> HistoryPage {
    let query = search.trim();
    let rows = repo::task_finished(db, query, page_size, 0);
    let total = repo::task_finished_count(db, query);
    let can_load_more = (rows.len() as i64) < total;

    // Group by completion day, newest day first.
    let mut grouped: BTreeMap<i64, Vec<FinishedTask>> = BTreeMap::new();
    for item in rows {
        let day_key = local_day_start(item.completed_at).round() as i64;
        grouped.entry(day_key).or_default().push(item);
    }

    let sections = grouped
        .into_iter()
        .rev()
        .map(|(day_key, items)| {
            let day = day_key as f64;
            let focus_seconds = items.iter().map(|i| i.focus_seconds).sum();
            HistorySection {
                day,
                title: format_history_day_title(day),
                items,
                focus_seconds,
            }
        })
        .collect();

    HistoryPage {
        sections,
        total,
        can_load_more,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{FocusSession, TaskItem};

    #[test]
    fn insights_and_history_summarize_week() {
        let db = Db::open_memory().unwrap();
        let today = today_start();
        let t = TaskItem {
            id: uuid::Uuid::new_v4().to_string().to_uppercase(),
            title: "Ship".into(),
            is_done: true,
            day: today,
            planned_day: today,
            time_limit_minutes: Some(25),
            remind_at: None,
            reminded: false,
            created_at: today,
            completed_at: Some(today + 3600.0),
        };
        repo::task_upsert(&db, &t, 0);
        repo::session_insert(
            &db,
            &FocusSession {
                task_id: Some(t.id.clone()),
                started_at: today + 1000.0,
                active_seconds: 1500,
                planned_seconds: 1500,
                completed: true,
            },
        );
        let summary = compute_insights(&db, 0);
        assert_eq!(summary.completed, 1);
        assert_eq!(summary.planned, 1);
        assert_eq!(summary.focus_seconds, 1500);
        assert_eq!(summary.headline, "Momentum is building");
        assert!(summary.sentence.contains("25m focused"));

        let hist = load_history(&db, "", 60);
        assert_eq!(hist.total, 1);
        assert_eq!(hist.sections[0].title, "Today");
        assert_eq!(hist.sections[0].focus_seconds, 1500);
    }
}
