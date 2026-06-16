use codex_collaboration_mode_templates::COMPOSE as COLLABORATION_MODE_COMPOSE;
use codex_collaboration_mode_templates::DEFAULT as COLLABORATION_MODE_DEFAULT;
use codex_collaboration_mode_templates::PLAN as COLLABORATION_MODE_PLAN;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::TUI_VISIBLE_COLLABORATION_MODES;
use codex_protocol::openai_models::ReasoningEffort;
use codex_utils_template::Template;
use std::sync::LazyLock;

const KNOWN_MODE_NAMES_TEMPLATE_KEY: &str = "KNOWN_MODE_NAMES";
const COMPOSE_SKILLS_TEMPLATE_KEY: &str = "COMPOSE_SKILLS";
static COLLABORATION_MODE_DEFAULT_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    Template::parse(COLLABORATION_MODE_DEFAULT)
        .unwrap_or_else(|err| panic!("collaboration mode default template must parse: {err}"))
});
static COLLABORATION_MODE_COMPOSE_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    Template::parse(COLLABORATION_MODE_COMPOSE)
        .unwrap_or_else(|err| panic!("collaboration mode compose template must parse: {err}"))
});

pub fn builtin_collaboration_mode_presets() -> Vec<CollaborationModeMask> {
    vec![default_preset(), plan_preset(), compose_preset()]
}

fn plan_preset() -> CollaborationModeMask {
    CollaborationModeMask {
        name: ModeKind::Plan.display_name().to_string(),
        mode: Some(ModeKind::Plan),
        model: None,
        reasoning_effort: Some(Some(ReasoningEffort::Medium)),
        developer_instructions: Some(Some(COLLABORATION_MODE_PLAN.to_string())),
    }
}

fn default_preset() -> CollaborationModeMask {
    CollaborationModeMask {
        name: ModeKind::Default.display_name().to_string(),
        mode: Some(ModeKind::Default),
        model: None,
        reasoning_effort: None,
        developer_instructions: Some(Some(default_mode_instructions())),
    }
}

fn compose_preset() -> CollaborationModeMask {
    CollaborationModeMask {
        name: ModeKind::Compose.display_name().to_string(),
        mode: Some(ModeKind::Compose),
        model: None,
        reasoning_effort: Some(Some(ReasoningEffort::Medium)),
        developer_instructions: Some(Some(compose_mode_instructions(""))),
    }
}

fn default_mode_instructions() -> String {
    let known_mode_names = format_mode_names(&TUI_VISIBLE_COLLABORATION_MODES);
    COLLABORATION_MODE_DEFAULT_TEMPLATE
        .render([(KNOWN_MODE_NAMES_TEMPLATE_KEY, known_mode_names.as_str())])
        .unwrap_or_else(|err| panic!("collaboration mode default template must render: {err}"))
}

pub(crate) fn compose_mode_instructions(compose_skills: &str) -> String {
    let known_mode_names = format_mode_names(&TUI_VISIBLE_COLLABORATION_MODES);
    COLLABORATION_MODE_COMPOSE_TEMPLATE
        .render([
            (KNOWN_MODE_NAMES_TEMPLATE_KEY, known_mode_names.as_str()),
            (COMPOSE_SKILLS_TEMPLATE_KEY, compose_skills),
        ])
        .unwrap_or_else(|err| panic!("collaboration mode compose template must render: {err}"))
}

fn format_mode_names(modes: &[ModeKind]) -> String {
    let mode_names: Vec<&str> = modes.iter().map(|mode| mode.display_name()).collect();
    match mode_names.as_slice() {
        [] => "none".to_string(),
        [mode_name] => (*mode_name).to_string(),
        [first, second] => format!("{first} and {second}"),
        [..] => mode_names.join(", "),
    }
}

#[cfg(test)]
#[path = "collaboration_mode_presets_tests.rs"]
mod tests;
