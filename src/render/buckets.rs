//! Due-date buckets for the open-task lists: the rule that places a task in a
//! bucket, the sums a bucket header shows, and the grouping of a task list.

use crate::i18n::t;
use chrono::{Datelike, Duration, NaiveDate};

/// Where a task's due date falls relative to today, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DueBucket {
    Overdue,
    Today,
    ThisWeek,
    Later,
    None,
}

impl DueBucket {
    pub const ALL: [DueBucket; 5] = [
        DueBucket::Overdue,
        DueBucket::Today,
        DueBucket::ThisWeek,
        DueBucket::Later,
        DueBucket::None,
    ];

    /// The stable machine key used in JSON output.
    pub fn key(self) -> &'static str {
        match self {
            DueBucket::Overdue => "overdue",
            DueBucket::Today => "today",
            DueBucket::ThisWeek => "this_week",
            DueBucket::Later => "later",
            DueBucket::None => "none",
        }
    }

    /// The translated header label.
    pub fn label(self) -> String {
        t(match self {
            DueBucket::Overdue => "Overdue",
            DueBucket::Today => "Today",
            DueBucket::ThisWeek => "This week",
            DueBucket::Later => "Later",
            DueBucket::None => "No due date",
        })
    }
}

/// The hour sums of one bucket; `None` is unknown.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BucketSums {
    pub estimate: Option<f64>,
    pub logged_week: Option<f64>,
    pub logged_total: Option<f64>,
}

/// A task as the bucket core sees it.
pub trait BucketedTask {
    fn due_on(&self) -> Option<&str>;
    fn project_id(&self) -> i64;
    fn task_number(&self) -> i64;
    fn estimate(&self) -> Option<f64>;
    fn logged_week(&self) -> Option<f64>;
    fn logged_total(&self) -> Option<f64>;
}

impl BucketedTask for super::MineTableRow {
    fn due_on(&self) -> Option<&str> {
        self.due_on.as_deref()
    }
    fn project_id(&self) -> i64 {
        self.project_id
    }
    fn task_number(&self) -> i64 {
        self.task_number
    }
    fn estimate(&self) -> Option<f64> {
        self.estimate
    }
    fn logged_week(&self) -> Option<f64> {
        self.logged_week
    }
    fn logged_total(&self) -> Option<f64> {
        self.logged_total
    }
}

/// One bucket with its tasks and sums.
#[derive(Debug)]
pub struct Bucket<'a, T> {
    pub bucket: DueBucket,
    pub items: Vec<&'a T>,
    pub sums: BucketSums,
}

fn parse_due(due_on: Option<&str>) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(due_on?, "%Y-%m-%d").ok()
}

/// The bucket of a `YYYY-MM-DD` due date; the week runs Monday to Sunday.
pub fn due_bucket(due_on: Option<&str>, today: NaiveDate) -> DueBucket {
    let Some(due) = parse_due(due_on) else {
        return DueBucket::None;
    };
    let sunday = today + Duration::days(6 - i64::from(today.weekday().num_days_from_monday()));
    if due < today {
        DueBucket::Overdue
    } else if due == today {
        DueBucket::Today
    } else if due <= sunday {
        DueBucket::ThisWeek
    } else {
        DueBucket::Later
    }
}

fn round_hours(hours: f64) -> f64 {
    (hours * 100.0).round() / 100.0
}

/// Sum of the known values, or unknown as soon as one value is unknown.
fn sum_all_known(values: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    let mut total = 0.0;
    for value in values {
        total += value?;
    }
    Some(round_hours(total))
}

/// The estimate adds the tasks that have one; week and total are unknown
/// when any task's value is, so a failed hours request never reads as a
/// smaller sum.
pub fn bucket_sums<T: BucketedTask>(items: &[&T]) -> BucketSums {
    let estimates: Vec<f64> = items.iter().filter_map(|i| i.estimate()).collect();
    BucketSums {
        estimate: (!estimates.is_empty()).then(|| round_hours(estimates.iter().sum())),
        logged_week: sum_all_known(items.iter().map(|i| i.logged_week())),
        logged_total: sum_all_known(items.iter().map(|i| i.logged_total())),
    }
}

/// Every task exactly once in the five buckets, in display order, each
/// sorted by due date, project id and task number (ties keep input order).
pub fn group_by_bucket<T: BucketedTask>(items: &[T], today: NaiveDate) -> Vec<Bucket<'_, T>> {
    DueBucket::ALL
        .iter()
        .map(|&bucket| {
            let mut members: Vec<&T> = items
                .iter()
                .filter(|i| due_bucket(i.due_on(), today) == bucket)
                .collect();
            members.sort_by_key(|i| (parse_due(i.due_on()), i.project_id(), i.task_number()));
            let sums = bucket_sums(&members);
            Bucket {
                bucket,
                items: members,
                sums,
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "../../tests/unit/buckets.rs"]
mod tests;
