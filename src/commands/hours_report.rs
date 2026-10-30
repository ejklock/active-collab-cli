use crate::client::{ActiveCollabClient, TimeRecordsOutcome};
use crate::i18n::t;
use crate::models::{TimeRecord, TimeRecordsReport};
use chrono::{DateTime, Datelike, Duration, NaiveDate};
use rust_xlsxwriter::{Format, Workbook};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

const BOM: &str = "\u{FEFF}";
const SEPARATOR: &str = ";";
const XLSX_DATE_FORMAT: &str = "dd/mm/yy";
const INPUT_DATE_FORMAT: &str = "%d-%m-%Y";

/// Hours with a decimal comma, at most two decimals, trailing zeros and a
/// bare comma dropped (`1`, `1,5`, `5,42`).
fn format_hours(hours: f64) -> String {
    let fixed = format!("{hours:.2}");
    fixed
        .trim_end_matches('0')
        .trim_end_matches('.')
        .replace('.', ",")
}

/// RFC 4180 field quoting: wrap when the field holds the separator, a double
/// quote, CR or LF, doubling inner quotes.
fn quote_field(field: &str) -> String {
    if field.contains([';', '"', '\r', '\n']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

struct ReportRow<'a> {
    record: &'a TimeRecord,
    project: &'a str,
    activity: &'a str,
}

fn report_rows(report: &TimeRecordsReport) -> Vec<ReportRow<'_>> {
    let projects: HashMap<i64, &str> = report
        .related
        .projects
        .iter()
        .map(|p| (p.id, p.name.as_str()))
        .collect();
    let tasks: HashMap<i64, (&str, i64)> = report
        .related
        .tasks
        .iter()
        .map(|task| (task.id, (task.name.as_str(), task.project_id)))
        .collect();
    let mut rows: Vec<ReportRow<'_>> = report
        .time_records
        .iter()
        .map(|record| {
            if record.parent_type == "Task" {
                let (name, project_id) = tasks.get(&record.parent_id).copied().unwrap_or(("", 0));
                ReportRow {
                    record,
                    project: projects.get(&project_id).copied().unwrap_or(""),
                    activity: name,
                }
            } else {
                ReportRow {
                    record,
                    project: projects.get(&record.parent_id).copied().unwrap_or(""),
                    activity: record.summary.as_str(),
                }
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        (a.record.record_date, a.project, a.record.id).cmp(&(
            b.record.record_date,
            b.project,
            b.record.id,
        ))
    });
    rows
}

fn total_hours(report: &TimeRecordsReport) -> f64 {
    report.time_records.iter().fold(0.0, |sum, r| sum + r.value)
}

/// `record_date` is a UTC-midnight timestamp; reading it in local time would
/// shift the date back a day west of UTC.
fn format_record_date(timestamp: i64) -> String {
    DateTime::from_timestamp(timestamp, 0)
        .map(|d| d.format("%d/%m/%y").to_string())
        .unwrap_or_default()
}

pub(crate) fn render_hours_report_csv(report: &TimeRecordsReport) -> String {
    let mut lines = vec![
        format!(
            "Total hours for the period{SEPARATOR}{}",
            format_hours(total_hours(report))
        ),
        String::new(),
        ["Project", "Activity", "Hours", "Date"].join(SEPARATOR),
    ];
    for row in report_rows(report) {
        lines.push(
            [
                quote_field(row.project),
                quote_field(row.activity),
                format_hours(row.record.value),
                format_record_date(row.record.record_date),
            ]
            .join(SEPARATOR),
        );
    }
    format!("{BOM}{}", lines.join("\n"))
}

pub(crate) fn render_hours_report_xlsx(report: &TimeRecordsReport) -> anyhow::Result<Vec<u8>> {
    let bold = Format::new().set_bold();
    let date_format = Format::new().set_num_format(XLSX_DATE_FORMAT);
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.set_name("Hours")?;
    sheet.write_string(0, 0, "Total hours for the period")?;
    sheet.write_number(0, 1, total_hours(report))?;
    for (col, title) in ["Project", "Activity", "Hours", "Date"].iter().enumerate() {
        sheet.write_string_with_format(2, col as u16, *title, &bold)?;
    }
    for (offset, row) in report_rows(report).iter().enumerate() {
        let line = 3 + offset as u32;
        sheet.write_string(line, 0, row.project)?;
        sheet.write_string(line, 1, row.activity)?;
        sheet.write_number(line, 2, row.record.value)?;
        if let Some(date) = DateTime::from_timestamp(row.record.record_date, 0) {
            sheet.write_datetime_with_format(line, 3, date.date_naive(), &date_format)?;
        }
    }
    sheet.autofit();
    Ok(workbook.save_to_buffer()?)
}

/// Parses a `DD-MM-YYYY` argument; rejects unpadded forms that chrono would
/// otherwise accept.
fn parse_input_date(raw: &str) -> Option<NaiveDate> {
    let date = NaiveDate::parse_from_str(raw, INPUT_DATE_FORMAT).ok()?;
    (date.format(INPUT_DATE_FORMAT).to_string() == raw).then_some(date)
}

/// A named period resolved against today's date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PeriodPreset {
    Month,
    Fortnight,
    Week,
}

/// Inclusive first and last day of the preset period containing `today`.
pub(crate) fn resolve_period(preset: PeriodPreset, today: NaiveDate) -> (NaiveDate, NaiveDate) {
    let month_start = today.with_day(1).unwrap_or(today);
    let month_end = || {
        let (year, month) = (today.year(), today.month());
        let (next_year, next_month) = if month == 12 {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };
        NaiveDate::from_ymd_opt(next_year, next_month, 1)
            .and_then(|first| first.pred_opt())
            .unwrap_or(today)
    };
    match preset {
        PeriodPreset::Month => (month_start, month_end()),
        PeriodPreset::Fortnight if today.day() <= 15 => {
            (month_start, today.with_day(15).unwrap_or(today))
        }
        PeriodPreset::Fortnight => (today.with_day(16).unwrap_or(today), month_end()),
        PeriodPreset::Week => {
            let monday = today - Duration::days(i64::from(today.weekday().num_days_from_monday()));
            (monday, monday + Duration::days(6))
        }
    }
}

fn fail(err: &mut dyn Write, message: &str, code: i32) -> i32 {
    writeln!(err, "Error: {message}").ok();
    code
}

/// The period of a report: typed `DD-MM-YYYY` bounds or a preset resolved
/// against today.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ReportPeriod<'a> {
    Range { start: &'a str, end: &'a str },
    Preset(PeriodPreset),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReportFormat {
    Xlsx,
    Csv,
}

impl ReportFormat {
    fn extension(self) -> &'static str {
        match self {
            ReportFormat::Xlsx => "xlsx",
            ReportFormat::Csv => "csv",
        }
    }

    fn render(self, report: &TimeRecordsReport) -> anyhow::Result<Vec<u8>> {
        match self {
            ReportFormat::Xlsx => render_hours_report_xlsx(report),
            ReportFormat::Csv => Ok(render_hours_report_csv(report).into_bytes()),
        }
    }
}

/// Everything a report run needs besides the client and the output streams.
/// `today` is the caller's local date; only presets read it.
pub(crate) struct ReportRequest<'a> {
    pub period: ReportPeriod<'a>,
    pub format: ReportFormat,
    pub today: NaiveDate,
    pub user_id: Option<i64>,
    pub out_dir: &'a Path,
}

/// A report period that owns its bounds, for effects that cross a task or
/// message boundary where the borrowed `ReportPeriod` cannot travel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OwnedReportPeriod {
    Range { start: String, end: String },
    Preset(PeriodPreset),
}

impl OwnedReportPeriod {
    pub(crate) fn as_period(&self) -> ReportPeriod<'_> {
        match self {
            OwnedReportPeriod::Range { start, end } => ReportPeriod::Range { start, end },
            OwnedReportPeriod::Preset(preset) => ReportPeriod::Preset(*preset),
        }
    }
}

/// Parses and orders a typed `DD-MM-YYYY` range; the error is the translated
/// message shown to the user.
pub(crate) fn validate_range(start: &str, end: &str) -> Result<(NaiveDate, NaiveDate), String> {
    let mut parsed = Vec::with_capacity(2);
    for raw in [start, end] {
        match parse_input_date(raw) {
            Some(date) => parsed.push(date),
            None => return Err(format!("{}: {raw}", t("invalid date, expected DD-MM-YYYY"))),
        }
    }
    if parsed[0] > parsed[1] {
        return Err(t("start date is after end date"));
    }
    Ok((parsed[0], parsed[1]))
}

fn resolve_report_dates(
    period: ReportPeriod<'_>,
    today: NaiveDate,
) -> Result<(NaiveDate, NaiveDate), String> {
    match period {
        ReportPeriod::Preset(preset) => Ok(resolve_period(preset, today)),
        ReportPeriod::Range { start, end } => validate_range(start, end),
    }
}

/// Writes through a sibling temporary file renamed over the target, so a
/// failed write never truncates an existing report and no temporary file
/// outlives the call.
fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut temp_name = path.file_name().unwrap_or_default().to_os_string();
    temp_name.push(".tmp");
    let temp = path.with_file_name(temp_name);
    let result = std::fs::write(&temp, bytes).and_then(|()| std::fs::rename(&temp, path));
    if result.is_err() {
        std::fs::remove_file(&temp).ok();
    }
    result
}

/// A written report: where it went and its formatted total.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExportedReport {
    pub path: PathBuf,
    pub total: String,
}

/// Why a report could not be produced. `Display` is the user-facing message
/// shared by the CLI and the TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExportError {
    Usage(String),
    MissingUserId,
    Unauthorized,
    Http(u16),
    Request(String),
    Render(String),
    Write { path: PathBuf, reason: String },
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::Usage(message) | ExportError::Request(message) => f.write_str(message),
            ExportError::MissingUserId => f.write_str(&t(
                "no user id stored for this instance; re-run `ac setup add`",
            )),
            ExportError::Unauthorized => f.write_str(&super::presenter::reauth_message()),
            ExportError::Http(status) => {
                write!(f, "{} {status}", t("hours report request failed with HTTP"))
            }
            ExportError::Render(reason) => {
                write!(f, "{}: {reason}", t("could not render the report"))
            }
            ExportError::Write { path, reason } => {
                write!(f, "{} {}: {reason}", t("could not write"), path.display())
            }
        }
    }
}

impl ExportError {
    /// CLI exit code: 2 for a usage error, 1 for any runtime failure.
    fn exit_code(&self) -> i32 {
        match self {
            ExportError::Usage(_) => 2,
            _ => 1,
        }
    }
}

/// Resolves the period, fetches the user's time records, renders them and
/// writes `hours-report-<start>-<end>.<xlsx|csv>` into the request's directory.
/// No file is created on any failure.
pub(crate) async fn export_hours_report(
    request: &ReportRequest<'_>,
    client: &ActiveCollabClient,
) -> Result<ExportedReport, ExportError> {
    let (from, to) =
        resolve_report_dates(request.period, request.today).map_err(ExportError::Usage)?;
    let user_id = match request.user_id {
        Some(id) if id != 0 => id,
        _ => return Err(ExportError::MissingUserId),
    };

    let outcome = client
        .fetch_time_records_by_date(
            user_id,
            &from.format("%Y-%m-%d").to_string(),
            &to.format("%Y-%m-%d").to_string(),
        )
        .await
        .map_err(|e| ExportError::Request(e.to_string()))?;
    let report = match outcome {
        TimeRecordsOutcome::Ok(report) => report,
        TimeRecordsOutcome::Unauthorized => return Err(ExportError::Unauthorized),
        TimeRecordsOutcome::Failed(status) => return Err(ExportError::Http(status)),
    };

    let bytes = request
        .format
        .render(&report)
        .map_err(|e| ExportError::Render(e.to_string()))?;
    let path = request.out_dir.join(format!(
        "hours-report-{}-{}.{}",
        from.format(INPUT_DATE_FORMAT),
        to.format(INPUT_DATE_FORMAT),
        request.format.extension()
    ));
    write_atomically(&path, &bytes).map_err(|e| ExportError::Write {
        path: path.clone(),
        reason: e.to_string(),
    })?;
    Ok(ExportedReport {
        path,
        total: format_hours(total_hours(&report)),
    })
}

/// Non-interactive hours report: resolves the period, fetches the user's
/// time records, and writes `hours-report-<start>-<end>.<xlsx|csv>` into the
/// request's directory. Returns an exit code: 0 success, 2 usage error, 1
/// runtime failure. No file is created on any failure.
pub(crate) async fn time_report_core(
    request: &ReportRequest<'_>,
    client: &ActiveCollabClient,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    match export_hours_report(request, client).await {
        Ok(exported) => {
            writeln!(
                out,
                "{} {} ({} {})",
                t("Hours report written to"),
                exported.path.display(),
                t("total hours:"),
                exported.total
            )
            .ok();
            0
        }
        Err(e) => fail(err, &e.to_string(), e.exit_code()),
    }
}
