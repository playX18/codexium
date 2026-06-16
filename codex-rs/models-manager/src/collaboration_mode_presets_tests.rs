use super::*;
use pretty_assertions::assert_eq;

#[test]
fn preset_names_use_mode_display_names() {
    assert_eq!(default_preset().name, ModeKind::Default.display_name());
    assert_eq!(plan_preset().name, ModeKind::Plan.display_name());
    assert_eq!(compose_preset().name, ModeKind::Compose.display_name());
    assert_eq!(plan_preset().model, None);
    assert_eq!(
        plan_preset().reasoning_effort,
        Some(Some(ReasoningEffort::Medium))
    );
    assert_eq!(default_preset().model, None);
    assert_eq!(default_preset().reasoning_effort, None);
    assert_eq!(compose_preset().model, None);
    assert_eq!(
        compose_preset().reasoning_effort,
        Some(Some(ReasoningEffort::Medium))
    );
}

#[test]
fn default_mode_instructions_replace_mode_names_placeholder() {
    let default_instructions = default_preset()
        .developer_instructions
        .expect("default preset should include instructions")
        .expect("default instructions should be set");

    assert!(!default_instructions.contains("{{KNOWN_MODE_NAMES}}"));

    let known_mode_names = format_mode_names(&TUI_VISIBLE_COLLABORATION_MODES);
    let expected_snippet = format!("Known mode names are {known_mode_names}.");
    assert!(default_instructions.contains(&expected_snippet));

    assert!(default_instructions.contains(
        "Use the `request_user_input` tool only when it is listed in the available tools"
    ));
    assert!(
        default_instructions.contains("ask the user directly with a concise plain-text question")
    );
}

#[test]
fn compose_mode_instructions_replace_placeholders() {
    let compose_instructions = compose_preset()
        .developer_instructions
        .expect("compose preset should include instructions")
        .expect("compose instructions should be set");

    assert!(!compose_instructions.contains("{{KNOWN_MODE_NAMES}}"));
    assert!(!compose_instructions.contains("{{COMPOSE_SKILLS}}"));

    let known_mode_names = format_mode_names(&TUI_VISIBLE_COLLABORATION_MODES);
    let expected_snippet = format!("Known mode names are {known_mode_names}.");
    assert!(compose_instructions.contains(&expected_snippet));
    assert!(compose_instructions.contains("Codex Compose Agent"));
    assert!(compose_instructions.contains("request_user_input"));
}
