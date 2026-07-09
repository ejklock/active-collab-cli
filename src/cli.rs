use clap::{Args, Parser, Subcommand};

pub const KNOWN_COMMANDS: [&str; 8] = [
    "setup", "get", "current", "mine", "list", "browse", "comment", "skill",
];

/// Fetch ActiveCollab tasks from one or more configured instances.
#[derive(Parser, Debug)]
#[command(name = "active-collab", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Manage instance configuration.
    Setup(SetupOpts),
    /// Fetch and display a task.
    Get(GetArgs),
    /// Fetch the task from the current git branch.
    Current(DisplayArgs),
    /// List open tasks assigned to you.
    #[command(alias = "list")]
    Mine(MineArgs),
    /// Interactive TUI browser for your tasks.
    Browse(BrowseArgs),
    /// Post a comment to a task as the logged-in user.
    Comment(CommentArgs),
    /// Print an embedded agent skill (ac skill list | ac skill <name>).
    Skill(SkillArgs),
}

/// Wrapper that holds the setup subcommand.
#[derive(Args, Debug)]
pub struct SetupOpts {
    #[command(subcommand)]
    pub subcommand: SetupCmd,
}

#[derive(Subcommand, Debug)]
pub enum SetupCmd {
    /// Register an ActiveCollab instance.
    Add(SetupAddArgs),
    /// List configured instances (no tokens).
    List,
    /// Remove an instance.
    Remove(RemoveArgs),
    /// Test connectivity.
    Test(TestArgs),
    /// Show or set the display language.
    Language(LanguageArgs),
    /// Show or set the color theme (angie, slate, nord).
    Theme(ThemeArgs),
}

#[derive(Args, Debug)]
pub struct SetupAddArgs {
    /// Unique name (prompted if omitted, interactive).
    #[arg(long)]
    pub name: Option<String>,
    /// Base URL, e.g. https://collab.example.com.
    #[arg(long)]
    pub url: Option<String>,
    /// Email for token exchange.
    #[arg(long)]
    pub email: Option<String>,
}

#[derive(Args, Debug)]
pub struct RemoveArgs {
    /// Name of the instance to remove.
    #[arg(long, required = true)]
    pub name: String,
}

#[derive(Args, Debug)]
pub struct TestArgs {
    /// Test only this instance.
    #[arg(long)]
    pub name: Option<String>,
}

#[derive(Args, Debug)]
pub struct LanguageArgs {
    /// Language code to set (en, pt_BR). Omit to show current.
    pub code: Option<String>,
}

#[derive(Args, Debug)]
pub struct ThemeArgs {
    /// Theme name: angie, slate, or nord. Omit to show current.
    pub code: Option<String>,
}

#[derive(Args, Debug)]
pub struct GetArgs {
    /// Task URL or PROJECT_ID/TASK_ID (e.g. 665/75159).
    pub ref_: String,
    #[command(flatten)]
    pub display: DisplayArgs,
}

#[derive(Args, Debug)]
pub struct DisplayArgs {
    /// Force a named instance.
    #[arg(long)]
    pub instance: Option<String>,
    /// Print PROJECT/TASK<TAB>name only.
    #[arg(long)]
    pub short: bool,
    /// Suppress comments.
    #[arg(long = "no-comments")]
    pub no_comments: bool,
    /// Print raw task JSON.
    #[arg(long)]
    pub json: bool,
    /// Ignore cache and re-fetch.
    #[arg(long)]
    pub refresh: bool,
}

#[derive(Args, Debug)]
pub struct MineArgs {
    /// Limit to this instance.
    #[arg(long)]
    pub instance: Option<String>,
    /// Print curated minified JSON for agent/LLM consumption; never launches the TUI.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct BrowseArgs {
    /// Force a named instance.
    #[arg(long)]
    pub instance: Option<String>,
    /// Print curated minified JSON for agent/LLM consumption; never launches the TUI.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct CommentArgs {
    /// Task URL or PROJECT_ID/TASK_ID. When omitted, resolved from the current git branch.
    pub task_ref: Option<String>,
    /// Comment body. When omitted, the body is read from stdin.
    #[arg(short = 'm', long)]
    pub message: Option<String>,
    /// Print curated minified JSON write result for agent/LLM consumption.
    #[arg(long)]
    pub json: bool,
    /// Force a named instance.
    #[arg(long)]
    pub instance: Option<String>,
}

#[derive(Args, Debug)]
pub struct SkillArgs {
    /// Skill name, or `list`. Omit to print the single skill.
    pub name: Option<String>,
}

/// Mirror of Python `_normalize_argv`.
///
/// - A first arg that is not a known command and not a `-` flag gets `"get"` prepended.
/// - Empty argv with a branch matching `^(feature|hotfix|fix)/<pid>-<tid>$` maps to `["current"]`.
/// - Everything else passes through unchanged.
///
/// `current_branch` is injected so the function remains pure and unit-testable.
pub fn normalize_argv(argv: &[String], current_branch: Option<&str>) -> Vec<String> {
    if let Some(first) = argv.first() {
        if !first.starts_with('-') && !KNOWN_COMMANDS.contains(&first.as_str()) {
            let mut out = vec!["get".to_owned()];
            out.extend_from_slice(argv);
            return out;
        }
        return argv.to_vec();
    }

    // argv is empty
    if let Some(branch) = current_branch {
        if branch_matches_task_pattern(branch) {
            return vec!["current".to_owned()];
        }
    }

    argv.to_vec()
}

/// Return true when `branch` matches `^(feature|hotfix|fix)/\d+-\d+$`.
/// Hand-parsed to avoid pulling in the `regex` crate.
fn branch_matches_task_pattern(branch: &str) -> bool {
    let rest = if let Some(r) = branch.strip_prefix("feature/") {
        r
    } else if let Some(r) = branch.strip_prefix("hotfix/") {
        r
    } else if let Some(r) = branch.strip_prefix("fix/") {
        r
    } else {
        return false;
    };

    // rest must match \d+-\d+  (no other characters)
    let Some(dash) = rest.find('-') else {
        return false;
    };
    let (left, right) = (&rest[..dash], &rest[dash + 1..]);
    !left.is_empty()
        && !right.is_empty()
        && left.chars().all(|c| c.is_ascii_digit())
        && right.chars().all(|c| c.is_ascii_digit())
}

/// Routing decision when `ac` is invoked with no subcommand.
#[derive(Debug, PartialEq)]
pub enum BareNoCommandAction {
    RunMine,
    HelpExit2,
}

/// Pure routing function: in a full TTY session launch the personal task view;
/// in a pipe or script fall back to help output so scripts are unaffected.
pub fn bare_no_command_action(is_tty: bool) -> BareNoCommandAction {
    if is_tty {
        BareNoCommandAction::RunMine
    } else {
        BareNoCommandAction::HelpExit2
    }
}

#[cfg(test)]
#[path = "../tests/unit/cli.rs"]
mod tests;
