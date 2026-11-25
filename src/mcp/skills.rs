//! The `active-collab` skill as the MCP skills extension serves it: one resource and one catalog
//! entry built from the text `ac skill active-collab` prints. No request argument reaches the
//! filesystem.

use crate::commands::SKILLS;
use rmcp::model::{Resource, Skill, SkillFrontmatter, SkillResource, SkillResources};

const SKILL_NAME: &str = "active-collab";
const MARKDOWN: &str = "text/markdown";
const FRONTMATTER_FENCE: &str = "---";

/// The embedded skill, built once at startup.
pub(super) struct SkillCatalog {
    text: &'static str,
    skill: Skill,
    resource: Resource,
}

impl SkillCatalog {
    pub(super) fn embedded() -> Result<Self, String> {
        let entry = SKILLS
            .iter()
            .find(|entry| entry.name == SKILL_NAME)
            .ok_or_else(|| format!("no embedded skill named {SKILL_NAME}"))?;
        Self::from_text(entry.body)
    }

    fn from_text(text: &'static str) -> Result<Self, String> {
        let frontmatter = parse_frontmatter(text)?;
        let uri = format!("skill://{}/SKILL.md", frontmatter.name);
        let resource = Resource::new(&uri, &frontmatter.name)
            .with_description(&frontmatter.description)
            .with_mime_type(MARKDOWN);
        let files = vec![SkillResource::from_bytes(&uri, text.as_bytes())];
        let skill = Skill::new(&uri, frontmatter, SkillResources::Files(files));
        Ok(Self {
            text,
            skill,
            resource,
        })
    }

    pub(super) fn uri(&self) -> &str {
        &self.skill.uri
    }

    pub(super) fn resources(&self) -> Vec<Resource> {
        vec![self.resource.clone()]
    }

    /// The skill text when `uri` is exactly the skill's URI.
    pub(super) fn read(&self, uri: &str) -> Option<&'static str> {
        (self.skill.uri == uri).then_some(self.text)
    }

    pub(super) fn skills(&self) -> Vec<Skill> {
        vec![self.skill.clone()]
    }

    pub(super) fn skill(&self, uri: &str) -> Option<&Skill> {
        (self.skill.uri == uri).then_some(&self.skill)
    }
}

/// Reads the `name:` and `description:` lines of the fenced block the skill opens with. The
/// block is the embedded file's own two plain single-line fields, so no YAML parser is needed.
fn parse_frontmatter(text: &str) -> Result<SkillFrontmatter, String> {
    let mut lines = text.lines();
    if lines.next() != Some(FRONTMATTER_FENCE) {
        return Err("the skill does not open with a frontmatter".to_string());
    }
    let block: Vec<&str> = lines
        .take_while(|line| *line != FRONTMATTER_FENCE)
        .collect();
    let field = |key: &str| {
        let prefix = format!("{key}: ");
        block
            .iter()
            .find_map(|line| line.strip_prefix(&prefix))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("the skill frontmatter needs a `{key}`"))
    };
    Ok(SkillFrontmatter::new(field("name")?, field("description")?))
}
