use codex_utils_absolute_path::AbsolutePathBuf;
use include_dir::Dir;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::Hash;
use std::hash::Hasher;

use crate::SystemSkillsError;

const COMPOSE_SKILLS_DIR: Dir = include_dir::include_dir!("$CARGO_MANIFEST_DIR/src/assets/compose");

const COMPOSE_SKILLS_DIR_NAME: &str = ".compose";
const SKILLS_DIR_NAME: &str = "skills";
const COMPOSE_SKILLS_MARKER_FILENAME: &str = ".codex-compose-skills.marker";
const COMPOSE_SKILLS_MARKER_SALT: &str = "v1";

/// Returns the on-disk cache location for embedded compose skills from an absolute CODEX_HOME.
pub fn compose_cache_root_dir(codex_home: &AbsolutePathBuf) -> AbsolutePathBuf {
    codex_home
        .join(SKILLS_DIR_NAME)
        .join(COMPOSE_SKILLS_DIR_NAME)
}

/// Installs embedded compose skills into `CODEX_HOME/skills/.compose`.
pub fn install_compose_skills(codex_home: &AbsolutePathBuf) -> Result<(), SystemSkillsError> {
    let skills_root_dir = codex_home.join(SKILLS_DIR_NAME);
    fs::create_dir_all(skills_root_dir.as_path())
        .map_err(|source| SystemSkillsError::io("create skills root dir", source))?;

    let dest_compose = compose_cache_root_dir(codex_home);

    let marker_path = dest_compose.join(COMPOSE_SKILLS_MARKER_FILENAME);
    let expected_fingerprint = embedded_compose_skills_fingerprint();
    if dest_compose.as_path().is_dir()
        && read_marker(&marker_path).is_ok_and(|marker| marker == expected_fingerprint)
    {
        return Ok(());
    }

    if dest_compose.as_path().exists() {
        fs::remove_dir_all(dest_compose.as_path()).map_err(|source| {
            SystemSkillsError::io("remove existing compose skills dir", source)
        })?;
    }

    write_embedded_dir(&COMPOSE_SKILLS_DIR, &dest_compose)?;
    fs::write(marker_path.as_path(), format!("{expected_fingerprint}\n"))
        .map_err(|source| SystemSkillsError::io("write compose skills marker", source))?;
    Ok(())
}

/// Build a `<compose_skills>` catalog block from compose skill metadata.
pub fn build_compose_skills_catalog<'a>(
    skills: impl IntoIterator<Item = (&'a str, &'a str, &'a str)>,
) -> String {
    let mut entries = Vec::new();
    for (name, description, location) in skills {
        if !name.starts_with("compose:") {
            continue;
        }
        entries.push(format!(
            "  <skill>\n    <name>{name}</name>\n    <description>{description}</description>\n    <location>{location}</location>\n  </skill>"
        ));
    }
    if entries.is_empty() {
        return String::new();
    }
    format!(
        "<compose_skills>\n{}\n</compose_skills>",
        entries.join("\n")
    )
}

fn read_marker(path: &AbsolutePathBuf) -> Result<String, SystemSkillsError> {
    Ok(fs::read_to_string(path.as_path())
        .map_err(|source| SystemSkillsError::io("read compose skills marker", source))?
        .trim()
        .to_string())
}

fn embedded_compose_skills_fingerprint() -> String {
    let mut items = Vec::new();
    collect_fingerprint_items(&COMPOSE_SKILLS_DIR, &mut items);
    items.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));

    let mut hasher = DefaultHasher::new();
    COMPOSE_SKILLS_MARKER_SALT.hash(&mut hasher);
    for (path, contents_hash) in items {
        path.hash(&mut hasher);
        contents_hash.hash(&mut hasher);
    }
    format!("{:x}", hasher.finish())
}

fn collect_fingerprint_items(dir: &Dir<'_>, items: &mut Vec<(String, Option<u64>)>) {
    for entry in dir.entries() {
        match entry {
            include_dir::DirEntry::Dir(subdir) => {
                items.push((subdir.path().to_string_lossy().to_string(), None));
                collect_fingerprint_items(subdir, items);
            }
            include_dir::DirEntry::File(file) => {
                let mut file_hasher = DefaultHasher::new();
                file.contents().hash(&mut file_hasher);
                items.push((
                    file.path().to_string_lossy().to_string(),
                    Some(file_hasher.finish()),
                ));
            }
        }
    }
}

fn write_embedded_dir(dir: &Dir<'_>, dest: &AbsolutePathBuf) -> Result<(), SystemSkillsError> {
    fs::create_dir_all(dest.as_path())
        .map_err(|source| SystemSkillsError::io("create compose skills dir", source))?;

    for entry in dir.entries() {
        match entry {
            include_dir::DirEntry::Dir(subdir) => {
                let subdir_dest = dest.join(subdir.path());
                fs::create_dir_all(subdir_dest.as_path()).map_err(|source| {
                    SystemSkillsError::io("create compose skills subdir", source)
                })?;
                write_embedded_dir(subdir, &subdir_dest)?;
            }
            include_dir::DirEntry::File(file) => {
                let path = dest.join(file.path());
                if let Some(parent) = path.as_path().parent() {
                    fs::create_dir_all(parent).map_err(|source| {
                        SystemSkillsError::io("create compose skills file parent", source)
                    })?;
                }
                fs::write(path.as_path(), file.contents())
                    .map_err(|source| SystemSkillsError::io("write compose skill file", source))?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_catalog_renders_skill_entries() {
        let catalog = build_compose_skills_catalog([(
            "compose:ask",
            "Ask the user",
            "file:///tmp/compose/ask/SKILL.md",
        )]);
        assert!(catalog.contains("<compose_skills>"));
        assert!(catalog.contains("<name>compose:ask</name>"));
    }
}
