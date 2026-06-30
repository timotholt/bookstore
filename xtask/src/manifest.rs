use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Default, Clone)]
pub struct SetupManifest {
    pub project: BTreeMap<String, String>,
    pub providers: Vec<ProviderManifest>,
}

impl SetupManifest {
    pub fn load(root: &Path) -> Result<Self, String> {
        let path = root.join("setup/setup.toml");
        let contents = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        Ok(parse_setup_manifest(&contents))
    }

    pub fn provider(&self, id: &str) -> Option<&ProviderManifest> {
        self.providers.iter().find(|provider| provider.id == id)
    }
}

#[derive(Debug, Default, Clone)]
pub struct ProviderManifest {
    pub id: String,
    pub adapter: String,
    pub required: bool,
    pub values: BTreeMap<String, String>,
    pub arrays: BTreeMap<String, Vec<String>>,
}

impl ProviderManifest {
    pub fn value(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn array(&self, key: &str) -> &[String] {
        self.arrays.get(key).map(Vec::as_slice).unwrap_or(&[])
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    None,
    Project,
    Provider(usize),
}

fn parse_setup_manifest(contents: &str) -> SetupManifest {
    let mut manifest = SetupManifest::default();
    let mut section = Section::None;

    for raw_line in contents.lines() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        match line {
            "[project]" => {
                section = Section::Project;
                continue;
            }
            "[[providers]]" => {
                manifest.providers.push(ProviderManifest::default());
                section = Section::Provider(manifest.providers.len() - 1);
                continue;
            }
            line if line.starts_with('[') => {
                section = Section::None;
                continue;
            }
            _ => {}
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();

        match section {
            Section::Project => {
                manifest
                    .project
                    .insert(key.to_string(), parse_scalar(value).unwrap_or_default());
            }
            Section::Provider(index) => {
                let provider = &mut manifest.providers[index];
                match key {
                    "id" => provider.id = parse_scalar(value).unwrap_or_default(),
                    "adapter" => provider.adapter = parse_scalar(value).unwrap_or_default(),
                    "required" => provider.required = value == "true",
                    _ if value.starts_with('[') => {
                        provider
                            .arrays
                            .insert(key.to_string(), parse_string_array(value));
                    }
                    _ => {
                        provider
                            .values
                            .insert(key.to_string(), parse_scalar(value).unwrap_or_default());
                    }
                }
            }
            Section::None => {}
        }
    }

    manifest
}

fn strip_comment(line: &str) -> &str {
    let mut in_quote = false;
    for (index, ch) in line.char_indices() {
        match ch {
            '"' => in_quote = !in_quote,
            '#' if !in_quote => return &line[..index],
            _ => {}
        }
    }
    line
}

fn parse_scalar(value: &str) -> Option<String> {
    let value = value.trim();
    if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        Some(value[1..value.len() - 1].to_string())
    } else if !value.starts_with('[') {
        Some(value.to_string())
    } else {
        None
    }
}

fn parse_string_array(value: &str) -> Vec<String> {
    let value = value.trim().trim_start_matches('[').trim_end_matches(']');
    value
        .split(',')
        .filter_map(|entry| parse_scalar(entry.trim()))
        .filter(|entry| !entry.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_project_and_provider_desired_state() {
        let manifest = parse_setup_manifest(
            r#"
            [project]
            name = "davis-books"

            [[providers]]
            id = "neon"
            adapter = "api"
            required = false
            project = "davis-books"
            roles = ["app", "migrator"]
            "#,
        );

        let neon = manifest.provider("neon").expect("neon provider");
        assert_eq!(manifest.project.get("name").unwrap(), "davis-books");
        assert_eq!(neon.adapter, "api");
        assert!(!neon.required);
        assert_eq!(neon.value("project"), Some("davis-books"));
        assert_eq!(neon.array("roles"), ["app", "migrator"]);
    }

    #[test]
    fn ignores_comments_outside_quotes() {
        let manifest = parse_setup_manifest(
            r#"
            [[providers]]
            id = "stripe" # provider id
            mode = "test # not a comment"
            "#,
        );

        let stripe = manifest.provider("stripe").expect("stripe provider");
        assert_eq!(stripe.value("mode"), Some("test # not a comment"));
    }
}
