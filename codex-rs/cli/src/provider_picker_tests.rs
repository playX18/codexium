use super::ProviderEntry;
use super::filter_provider_indices;
use super::provider_entry_match_score;
use pretty_assertions::assert_eq;

fn entry(id: &str, name: &str) -> ProviderEntry {
    ProviderEntry {
        id: id.to_string(),
        name: name.to_string(),
    }
}

#[test]
fn filter_provider_indices_returns_all_when_query_empty() {
    let entries = vec![entry("z-provider", "Zed"), entry("a-provider", "Alpha")];
    assert_eq!(filter_provider_indices(&entries, ""), vec![0, 1]);
}

#[test]
fn filter_provider_indices_fuzzy_matches_id_and_name() {
    let entries = vec![
        entry("anthropic", "Anthropic"),
        entry("openrouter", "OpenRouter"),
        entry("xiaomi-token-plan-sgp", "Xiaomi Token Plan (Singapore)"),
    ];
    let matches = filter_provider_indices(&entries, "anth");
    assert_eq!(matches, vec![0]);
    let matches = filter_provider_indices(&entries, "sing");
    assert_eq!(matches, vec![2]);
}

#[test]
fn filter_provider_indices_sorts_by_match_quality_then_id() {
    let entries = vec![
        entry("my-anthropic", "Other"),
        entry("anthropic", "Anthropic"),
    ];
    let matches = filter_provider_indices(&entries, "anthropic");
    assert_eq!(matches, vec![1, 0]);
}

#[test]
fn provider_entry_match_score_prefers_exact_prefix() {
    let exact = entry("anthropic", "Anthropic");
    let partial = entry("my-anthropic", "Other");
    let score_exact = provider_entry_match_score(&exact, "anthropic").expect("match");
    let score_partial = provider_entry_match_score(&partial, "anthropic").expect("match");
    assert!(score_exact < score_partial);
}
