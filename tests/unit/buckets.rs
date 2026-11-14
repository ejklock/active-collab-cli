use super::*;

fn day(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
}

#[test]
fn due_bucket_on_a_wednesday_splits_at_today_and_the_coming_sunday() {
    let today = day(2026, 11, 11);
    assert_eq!(due_bucket(Some("2026-11-10"), today), DueBucket::Overdue);
    assert_eq!(due_bucket(Some("2026-11-11"), today), DueBucket::Today);
    assert_eq!(due_bucket(Some("2026-11-12"), today), DueBucket::ThisWeek);
    assert_eq!(due_bucket(Some("2026-11-15"), today), DueBucket::ThisWeek);
    assert_eq!(due_bucket(Some("2026-11-16"), today), DueBucket::Later);
}

#[test]
fn due_bucket_treats_missing_and_malformed_dates_as_no_due_date() {
    let today = day(2026, 11, 11);
    assert_eq!(due_bucket(None, today), DueBucket::None);
    assert_eq!(due_bucket(Some("not-a-date"), today), DueBucket::None);
    assert_eq!(due_bucket(Some("2026-13-01"), today), DueBucket::None);
}

#[test]
fn due_bucket_on_a_sunday_has_no_this_week_left() {
    let today = day(2026, 11, 15);
    assert_eq!(due_bucket(Some("2026-11-15"), today), DueBucket::Today);
    assert_eq!(due_bucket(Some("2026-11-16"), today), DueBucket::Later);
    assert_eq!(due_bucket(Some("2026-11-14"), today), DueBucket::Overdue);
}

#[test]
fn due_bucket_week_crosses_the_year_boundary() {
    let today = day(2025, 12, 31);
    assert_eq!(due_bucket(Some("2026-01-04"), today), DueBucket::ThisWeek);
    assert_eq!(due_bucket(Some("2026-01-05"), today), DueBucket::Later);
    assert_eq!(due_bucket(Some("2025-12-30"), today), DueBucket::Overdue);
}

#[test]
fn due_bucket_keys_and_order_are_stable() {
    let keys: Vec<&str> = DueBucket::ALL.iter().map(|b| b.key()).collect();
    assert_eq!(keys, ["overdue", "today", "this_week", "later", "none"]);
}

#[derive(Debug, Clone)]
struct Item {
    due: Option<&'static str>,
    project: i64,
    number: i64,
    estimate: Option<f64>,
    week: Option<f64>,
    total: Option<f64>,
}

impl BucketedTask for Item {
    fn due_on(&self) -> Option<&str> {
        self.due
    }
    fn project_id(&self) -> i64 {
        self.project
    }
    fn task_number(&self) -> i64 {
        self.number
    }
    fn estimate(&self) -> Option<f64> {
        self.estimate
    }
    fn logged_week(&self) -> Option<f64> {
        self.week
    }
    fn logged_total(&self) -> Option<f64> {
        self.total
    }
}

fn item(estimate: Option<f64>, week: Option<f64>, total: Option<f64>) -> Item {
    Item {
        due: None,
        project: 1,
        number: 1,
        estimate,
        week,
        total,
    }
}

fn sums_of(items: &[Item]) -> BucketSums {
    let refs: Vec<&Item> = items.iter().collect();
    bucket_sums(&refs)
}

#[test]
fn bucket_sums_estimate_adds_only_the_tasks_that_have_one() {
    let items = [
        item(Some(2.0), None, None),
        item(None, None, None),
        item(Some(1.5), None, None),
    ];
    assert_eq!(sums_of(&items).estimate, Some(3.5));
}

#[test]
fn bucket_sums_estimate_is_unknown_when_no_task_has_one() {
    let items = [item(None, None, None), item(None, None, None)];
    assert_eq!(sums_of(&items).estimate, None);
    assert_eq!(sums_of(&[]).estimate, None);
}

#[test]
fn bucket_sums_week_and_total_add_when_every_value_is_known() {
    let items = [
        item(None, Some(1.0), Some(0.0)),
        item(None, Some(2.5), Some(0.0)),
    ];
    let sums = sums_of(&items);
    assert_eq!(sums.logged_week, Some(3.5));
    assert_eq!(sums.logged_total, Some(0.0));
}

#[test]
fn bucket_sums_week_and_total_are_unknown_as_soon_as_one_value_is() {
    let items = [
        item(None, Some(1.0), None),
        item(None, None, Some(4.0)),
        item(None, Some(2.0), Some(1.0)),
    ];
    let sums = sums_of(&items);
    assert_eq!(sums.logged_week, None);
    assert_eq!(sums.logged_total, None);
}

#[test]
fn bucket_sums_week_and_total_judge_each_column_on_its_own() {
    let items = [
        item(None, Some(1.0), None),
        item(None, Some(2.0), Some(3.0)),
    ];
    let sums = sums_of(&items);
    assert_eq!(sums.logged_week, Some(3.0));
    assert_eq!(sums.logged_total, None);
}

#[test]
fn bucket_sums_of_an_empty_bucket_are_zero_hours() {
    let sums = sums_of(&[]);
    assert_eq!(sums.logged_week, Some(0.0));
    assert_eq!(sums.logged_total, Some(0.0));
}

#[test]
fn bucket_sums_round_to_two_decimals() {
    let items = [
        item(Some(1.1), Some(1.1), Some(1.1)),
        item(Some(2.2), Some(2.2), Some(2.2)),
    ];
    let sums = sums_of(&items);
    assert_eq!(sums.estimate, Some(3.3));
    assert_eq!(sums.logged_week, Some(3.3));
    assert_eq!(sums.logged_total, Some(3.3));
}

fn dated(due: Option<&'static str>, project: i64, number: i64) -> Item {
    Item {
        due,
        project,
        number,
        estimate: None,
        week: None,
        total: None,
    }
}

#[test]
fn bucket_sums_grouping_returns_five_buckets_in_order_with_every_task_once() {
    let today = day(2026, 11, 11);
    let items = vec![
        dated(Some("2026-11-20"), 1, 1),
        dated(None, 1, 2),
        dated(Some("2026-11-10"), 1, 3),
        dated(Some("2026-11-11"), 1, 4),
        dated(Some("2026-11-13"), 1, 5),
    ];
    let groups = group_by_bucket(&items, today);
    let order: Vec<DueBucket> = groups.iter().map(|g| g.bucket).collect();
    assert_eq!(order, DueBucket::ALL);
    let counts: Vec<usize> = groups.iter().map(|g| g.items.len()).collect();
    assert_eq!(counts, [1, 1, 1, 1, 1]);
    let numbers: Vec<i64> = groups
        .iter()
        .flat_map(|g| g.items.iter().map(|i| i.number))
        .collect();
    assert_eq!(numbers, [3, 4, 5, 1, 2]);
}

#[test]
fn bucket_sums_grouping_sorts_by_due_then_project_then_number_keeping_ties_in_input_order() {
    let today = day(2026, 11, 1);
    let mut tie_a = dated(Some("2026-11-20"), 2, 7);
    tie_a.estimate = Some(1.0);
    let mut tie_b = dated(Some("2026-11-20"), 2, 7);
    tie_b.estimate = Some(2.0);
    let items = vec![
        tie_a,
        dated(Some("2026-11-21"), 1, 1),
        dated(Some("2026-11-20"), 2, 3),
        dated(Some("2026-11-20"), 1, 9),
        tie_b,
    ];
    let groups = group_by_bucket(&items, today);
    let later = &groups[3];
    let order: Vec<(Option<&str>, i64, i64, Option<f64>)> = later
        .items
        .iter()
        .map(|i| (i.due, i.project, i.number, i.estimate))
        .collect();
    assert_eq!(
        order,
        [
            (Some("2026-11-20"), 1, 9, None),
            (Some("2026-11-20"), 2, 3, None),
            (Some("2026-11-20"), 2, 7, Some(1.0)),
            (Some("2026-11-20"), 2, 7, Some(2.0)),
            (Some("2026-11-21"), 1, 1, None),
        ]
    );
}

#[test]
fn bucket_sums_grouping_attaches_each_bucket_its_own_sums() {
    let today = day(2026, 11, 11);
    let mut overdue = dated(Some("2026-11-01"), 1, 1);
    overdue.estimate = Some(2.0);
    overdue.week = Some(1.0);
    overdue.total = Some(3.0);
    let mut unknown_week = dated(None, 1, 2);
    unknown_week.total = Some(1.0);
    let items = vec![overdue, unknown_week];
    let groups = group_by_bucket(&items, today);
    assert_eq!(groups[0].sums.estimate, Some(2.0));
    assert_eq!(groups[0].sums.logged_week, Some(1.0));
    assert_eq!(groups[1].sums.logged_week, Some(0.0));
    assert_eq!(groups[4].sums.logged_week, None);
    assert_eq!(groups[4].sums.logged_total, Some(1.0));
}
