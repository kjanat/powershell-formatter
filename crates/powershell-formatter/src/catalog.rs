//! Command/parameter casing catalogs.
//!
//! PSScriptAnalyzer resolves canonical command casing from a live runspace;
//! this formatter never runs PowerShell, so casing data is *injected*. Without
//! a catalog, keyword and operator casing still work and formatting stays
//! deterministic.

use std::collections::HashMap;

/// Source of canonical command and parameter spellings.
pub trait CommandCatalog {
    /// Canonical spelling for a command name (e.g. `get-childitem` →
    /// `Get-ChildItem`), or `None` when unknown (leave as written).
    fn canonical_command(&self, name: &str) -> Option<&str>;

    /// Canonical spelling for `-parameter` of `command` (without the dash),
    /// or `None` when unknown.
    fn canonical_parameter(&self, command: &str, parameter: &str) -> Option<&str>;
}

/// A catalog backed by an in-memory map, loadable from JSON.
///
/// JSON shape (canonical casing is taken from the spellings used):
///
/// ```json
/// { "commands": { "Get-ChildItem": ["Path", "Filter", "Recurse"] } }
/// ```
#[derive(Debug, Default, Clone)]
pub struct JsonCatalog {
    /// lowercase command name → (canonical name, lowercase param → canonical).
    commands: HashMap<String, (String, HashMap<String, String>)>,
}

impl JsonCatalog {
    /// Parse a catalog from its JSON representation.
    ///
    /// # Errors
    /// Returns a message when the JSON is malformed or not the expected
    /// shape.
    #[cfg(feature = "serde")]
    pub fn from_json(json: &str) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Raw {
            commands: HashMap<String, Vec<String>>,
        }
        let raw: Raw = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let mut commands = HashMap::with_capacity(raw.commands.len());
        for (name, params) in raw.commands {
            let param_map = params.into_iter().map(|p| (p.to_lowercase(), p)).collect();
            commands.insert(name.to_lowercase(), (name, param_map));
        }
        Ok(Self { commands })
    }

    /// Insert one command with its parameters (canonical spellings).
    pub fn insert(&mut self, command: &str, parameters: &[&str]) {
        let params = parameters
            .iter()
            .map(|p| ((*p).to_lowercase(), (*p).to_owned()))
            .collect();
        self.commands
            .insert(command.to_lowercase(), (command.to_owned(), params));
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

impl CommandCatalog for JsonCatalog {
    fn canonical_command(&self, name: &str) -> Option<&str> {
        self.commands
            .get(&name.to_lowercase())
            .map(|(canonical, _)| canonical.as_str())
    }

    fn canonical_parameter(&self, command: &str, parameter: &str) -> Option<&str> {
        self.commands
            .get(&command.to_lowercase())
            .and_then(|(_, params)| params.get(&parameter.to_lowercase()))
            .map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_is_case_insensitive() {
        let mut c = JsonCatalog::default();
        c.insert("Get-ChildItem", &["Path", "LiteralPath"]);
        assert_eq!(c.canonical_command("get-childitem"), Some("Get-ChildItem"));
        assert_eq!(c.canonical_command("GET-CHILDITEM"), Some("Get-ChildItem"));
        assert_eq!(c.canonical_command("Get-Item"), None);
        assert_eq!(
            c.canonical_parameter("GET-childitem", "literalpath"),
            Some("LiteralPath")
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn json_round_trip() {
        let c = JsonCatalog::from_json(
            r#"{ "commands": { "Write-Output": ["InputObject", "NoEnumerate"] } }"#,
        )
        .unwrap();
        assert_eq!(c.canonical_command("write-output"), Some("Write-Output"));
        assert_eq!(
            c.canonical_parameter("write-output", "noenumerate"),
            Some("NoEnumerate")
        );
        assert!(JsonCatalog::from_json("not json").is_err());
    }
}
