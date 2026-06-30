use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvValue {
    pub value: String,
    pub source: EnvSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnvSource {
    Process,
    SecretsDemo,
    DotEnvLocal,
    DotEnv,
}

impl EnvSource {
    pub fn label(&self) -> &'static str {
        match self {
            EnvSource::Process => "process environment",
            EnvSource::SecretsDemo => "setup/.secrets.demo.env",
            EnvSource::DotEnvLocal => ".env.local",
            EnvSource::DotEnv => ".env",
        }
    }
}

pub struct EnvStore {
    values: BTreeMap<String, EnvValue>,
}

impl EnvStore {
    pub fn load(root: &Path) -> Self {
        let mut values = BTreeMap::new();
        load_file(&root.join(".env"), EnvSource::DotEnv, &mut values);
        load_file(
            &root.join(".env.local"),
            EnvSource::DotEnvLocal,
            &mut values,
        );
        load_file(
            &root.join("setup/.secrets.demo.env"),
            EnvSource::SecretsDemo,
            &mut values,
        );

        for (name, value) in env::vars() {
            values.insert(
                name,
                EnvValue {
                    value,
                    source: EnvSource::Process,
                },
            );
        }

        Self { values }
    }

    pub fn get(&self, name: &str) -> Option<&EnvValue> {
        self.values.get(name)
    }

    #[cfg(test)]
    pub fn from_values(values: BTreeMap<String, EnvValue>) -> Self {
        Self { values }
    }
}

fn load_file(path: &Path, source: EnvSource, values: &mut BTreeMap<String, EnvValue>) {
    let Ok(contents) = fs::read_to_string(path) else {
        return;
    };

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with("export ") && !line.contains('=')
        {
            continue;
        }

        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }

        values.insert(
            name.to_string(),
            EnvValue {
                value: unquote(value.trim()).to_string(),
                source: source.clone(),
            },
        );
    }
}

fn unquote(value: &str) -> &str {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return &value[1..value.len() - 1];
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_dotenv_lines_and_quotes() {
        let dir = temp_dir("dotenv_parse");
        fs::write(
            dir.join(".env"),
            "TEST_DATABASE_URL_FOR_PARSE='postgresql://localhost/davis_books'\nexport APP_ENV=local\n# ignored\n",
        )
        .unwrap();

        let store = EnvStore::load(&dir);
        assert_eq!(
            store.get("TEST_DATABASE_URL_FOR_PARSE").unwrap().value,
            "postgresql://localhost/davis_books"
        );
        assert_eq!(store.get("APP_ENV").unwrap().value, "local");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn secrets_file_overrides_dotenv() {
        let dir = temp_dir("dotenv_priority");
        fs::create_dir_all(dir.join("setup")).unwrap();
        fs::write(dir.join(".env"), "SESSION_SECRET=from-env\n").unwrap();
        fs::write(
            dir.join("setup/.secrets.demo.env"),
            "SESSION_SECRET=from-secrets\n",
        )
        .unwrap();

        let store = EnvStore::load(&dir);
        let value = store.get("SESSION_SECRET").unwrap();
        assert_eq!(value.value, "from-secrets");
        assert_eq!(value.source, EnvSource::SecretsDemo);

        let _ = fs::remove_dir_all(dir);
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("davis_books_xtask_{name}_{nanos}"));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
