use crate::SkillLoadOutcome;
use crate::SkillMetadata;
use crate::SkillMetadataBudget;
use crate::SkillPolicy;
use crate::build_available_skills;
use crate::render::SkillRenderSideEffects;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

fn compose_skill(name: &str) -> SkillMetadata {
    SkillMetadata {
        name: name.to_string(),
        description: format!("{name} description"),
        short_description: None,
        interface: None,
        dependencies: None,
        policy: Some(SkillPolicy {
            allow_implicit_invocation: Some(false),
            products: Vec::new(),
        }),
        path_to_skills_md: AbsolutePathBuf::try_from(format!("/tmp/{name}/SKILL.md"))
            .expect("valid path"),
        scope: SkillScope::System,
        plugin_id: None,
    }
}

fn regular_skill(name: &str) -> SkillMetadata {
    SkillMetadata {
        name: name.to_string(),
        description: format!("{name} description"),
        short_description: None,
        interface: None,
        dependencies: None,
        policy: None,
        path_to_skills_md: AbsolutePathBuf::try_from(format!("/tmp/{name}/SKILL.md"))
            .expect("valid path"),
        scope: SkillScope::System,
        plugin_id: None,
    }
}

#[test]
fn compose_skills_are_excluded_from_available_skills_outside_compose_mode() {
    let mut outcome = SkillLoadOutcome::default();
    outcome.skills = vec![compose_skill("compose:ask"), regular_skill("regular-skill")];

    let rendered = build_available_skills(
        &outcome,
        SkillMetadataBudget::Characters(usize::MAX),
        SkillRenderSideEffects::None,
        false,
    )
    .expect("skills should render");

    assert_eq!(rendered.report.included_count, 1);
    let rendered_text = rendered.skill_lines.join("\n");
    assert!(rendered_text.contains("regular-skill"));
    assert!(!rendered_text.contains("compose:ask"));
}

#[test]
fn compose_skills_are_listed_in_available_skills_in_compose_mode() {
    let mut outcome = SkillLoadOutcome::default();
    outcome.skills = vec![compose_skill("compose:ask")];

    let rendered = build_available_skills(
        &outcome,
        SkillMetadataBudget::Characters(usize::MAX),
        SkillRenderSideEffects::None,
        true,
    )
    .expect("skills should render");

    assert_eq!(rendered.report.included_count, 1);
    assert!(rendered.skill_lines.join("\n").contains("compose:ask"));
}
