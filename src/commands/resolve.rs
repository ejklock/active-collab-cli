use crate::i18n::t;
use crate::store::instances::Instance;
use regex::Regex;
use std::io::Write;
use std::sync::OnceLock;

/// Parity: Python _pick_instance — exported for reuse by R4/R5.
///
/// Returns the single matching Instance or an i32 exit code (always 2) on failure.
/// Error messages go to `err`.
#[allow(dead_code)]
pub(crate) fn pick_instance(
    instances: &[Instance],
    name: Option<&str>,
    err: &mut dyn Write,
) -> Result<usize, i32> {
    if instances.is_empty() {
        writeln!(
            err,
            "{}",
            t("Error: no instances configured. Run: active_collab.py setup add")
        )
        .ok();
        return Err(2);
    }

    if let Some(n) = name {
        let pos = instances.iter().position(|i| i.name == n);
        match pos {
            Some(idx) => return Ok(idx),
            None => {
                let known: Vec<&str> = instances.iter().map(|i| i.name.as_str()).collect();
                let known_str = known.join(", ");
                writeln!(
                    err,
                    "{}",
                    t(&format!(
                        "Error: instance '{name}' not found. Known: {known}",
                        name = n,
                        known = known_str
                    ))
                )
                .ok();
                return Err(2);
            }
        }
    }

    if instances.len() == 1 {
        return Ok(0);
    }

    let names: Vec<&str> = instances.iter().map(|i| i.name.as_str()).collect();
    let names_str = names.join(", ");
    writeln!(
        err,
        "{}",
        t(&format!(
            "Error: multiple instances configured ({names}). Use --instance NAME.",
            names = names_str
        ))
    )
    .ok();
    Err(2)
}

fn task_url_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"/projects/(\d+)/tasks/(\d+)").expect("task_url_re is a valid pattern")
    })
}

fn branch_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(feature|hotfix|fix)/(\d+)-(\d+)$").expect("branch_re is a valid pattern")
    })
}

/// Parity: Python _parse_task_ref.
///
/// Returns (project_id, task_id) from a URL or "P/T" digit-slash-digit form.
/// Writes an error and returns Err(2) on bad input.
pub(crate) fn parse_task_ref(ref_: &str, err: &mut dyn Write) -> Result<(i64, i64), i32> {
    if let Some(caps) = task_url_re().captures(ref_) {
        let pid: i64 = caps[1].parse().unwrap();
        let tid: i64 = caps[2].parse().unwrap();
        return Ok((pid, tid));
    }

    let parts: Vec<&str> = ref_.split('/').collect();
    if parts.len() == 2 {
        if let (Ok(pid), Ok(tid)) = (parts[0].parse::<i64>(), parts[1].parse::<i64>()) {
            return Ok((pid, tid));
        }
    }

    writeln!(
        err,
        "{}",
        t(&format!(
            "Error: cannot parse task ref '{ref}'. Use URL or PROJECT_ID/TASK_ID (e.g. 665/75159).",
            ref = ref_
        ))
    )
    .ok();
    Err(2)
}

/// Parity: Python _parse_branch_ref.
///
/// Returns Some((project_id, task_id)) when branch matches
/// `^(feature|hotfix|fix)/<pid>-<tid>$`, else None.
pub(crate) fn parse_branch_ref(branch: &str) -> Option<(i64, i64)> {
    let caps = branch_re().captures(branch)?;
    let pid: i64 = caps[2].parse().ok()?;
    let tid: i64 = caps[3].parse().ok()?;
    Some((pid, tid))
}

pub(crate) fn resolve_task_ref_for_comment(
    task_ref: Option<&str>,
    branch: Option<&str>,
    err: &mut dyn Write,
) -> Result<(i64, i64), i32> {
    if let Some(r) = task_ref {
        return parse_task_ref(r, err);
    }
    let branch = match branch {
        Some(b) => b,
        None => {
            writeln!(
                err,
                "{}",
                t("Error: no task ref given and not in a git repository or HEAD is detached.")
            )
            .ok();
            return Err(2);
        }
    };
    match parse_branch_ref(branch) {
        Some(ids) => Ok(ids),
        None => {
            writeln!(
                err,
                "{}",
                t(&format!(
                    "Error: no task ref given and branch '{branch}' does not match \
                     expected pattern (feature|hotfix|fix)/PROJECT_ID-TASK_ID.",
                    branch = branch
                ))
            )
            .ok();
            Err(2)
        }
    }
}
