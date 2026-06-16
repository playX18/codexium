use codex_skills::build_compose_skills_catalog;

use crate::model::SkillLoadOutcome;

pub(crate) use codex_skills::compose_cache_root_dir;
pub(crate) use codex_skills::install_compose_skills;

/// Build a `<compose_skills>` catalog from loaded skill metadata.
pub fn build_compose_skills_catalog_from_outcome(outcome: &SkillLoadOutcome) -> String {
    let entries: Vec<(String, String, String)> = outcome
        .skills
        .iter()
        .filter(|skill| skill.name.starts_with("compose:") && outcome.is_skill_enabled(skill))
        .map(|skill| {
            (
                skill.name.clone(),
                skill.description.clone(),
                skill.path_to_skills_md.display().to_string(),
            )
        })
        .collect();

    build_compose_skills_catalog(entries.iter().map(|(name, description, location)| {
        (name.as_str(), description.as_str(), location.as_str())
    }))
}
