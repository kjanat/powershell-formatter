//! dprint Wasm plugin for PowerShell.
//!
//! This crate is protocol plumbing only: configuration resolution and the
//! dprint plugin ABI. All formatting policy lives in `pwsh-formatter`.

use dprint_core::configuration::{
    ConfigKeyMap, ConfigKeyValue, ConfigurationDiagnostic, GlobalConfiguration, NewLineKind,
};
use dprint_core::plugins::{
    CheckConfigUpdatesMessage, ConfigChange, FileMatchingInfo, FormatError, FormatResult,
    PluginInfo, PluginResolveConfigurationResult, SyncFormatRequest, SyncHostFormatRequest,
    SyncPluginHandler,
};
use pwsh_formatter::{EndOfLine, FormatOptions};

/// The resolved plugin configuration: the core options plus nothing else —
/// the adapter adds no policy of its own.
pub type Configuration = FormatOptions;

pub struct PowerShellPluginHandler;

impl PowerShellPluginHandler {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for PowerShellPluginHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a dprint config value into JSON for overlaying onto the options.
fn key_value_to_json(value: &ConfigKeyValue) -> serde_json::Value {
    match value {
        ConfigKeyValue::String(s) => serde_json::Value::String(s.clone()),
        ConfigKeyValue::Number(n) => serde_json::Value::Number((*n).into()),
        ConfigKeyValue::Bool(b) => serde_json::Value::Bool(*b),
        ConfigKeyValue::Array(items) => {
            serde_json::Value::Array(items.iter().map(key_value_to_json).collect())
        }
        ConfigKeyValue::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), key_value_to_json(v)))
                .collect(),
        ),
        ConfigKeyValue::Null => serde_json::Value::Null,
    }
}

/// Resolve the user's `dprint.json` slice into [`FormatOptions`].
///
/// Known keys are exactly the serde surface of [`FormatOptions`] — derived at
/// runtime from the type itself so the plugin can never drift from the core.
/// Global `lineWidth`/`indentWidth`/`useTabs`/`newLineKind` are inherited,
/// with plugin-specific keys overriding them.
pub fn resolve_config(
    config: ConfigKeyMap,
    global_config: &GlobalConfiguration,
) -> PluginResolveConfigurationResult<Configuration> {
    let mut diagnostics = Vec::<ConfigurationDiagnostic>::new();

    let mut options = FormatOptions::default();
    if let Some(width) = global_config.line_width {
        options.line_width = u16::try_from(width).unwrap_or(u16::MAX);
    }
    if let Some(indent) = global_config.indent_width {
        options.indent_width = indent;
    }
    if let Some(tabs) = global_config.use_tabs {
        options.use_tabs = tabs;
    }
    if let Some(kind) = global_config.new_line_kind {
        options.end_of_line = match kind {
            NewLineKind::LineFeed => EndOfLine::Lf,
            NewLineKind::CarriageReturnLineFeed => EndOfLine::Crlf,
            _ => EndOfLine::Auto,
        };
    }

    // The canonical key set comes from serializing the options themselves.
    let mut object = match serde_json::to_value(&options) {
        Ok(serde_json::Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    };

    for (key, value) in &config {
        if !object.contains_key(key) {
            diagnostics.push(ConfigurationDiagnostic {
                property_name: key.clone(),
                message: format!("Unknown property in configuration: {key}"),
            });
            continue;
        }
        // Validate each key independently so one bad value cannot poison
        // the rest of the configuration.
        let mut candidate = object.clone();
        candidate.insert(key.clone(), key_value_to_json(value));
        match serde_json::from_value::<FormatOptions>(serde_json::Value::Object(candidate.clone()))
        {
            Ok(_) => {
                object = candidate;
            }
            Err(err) => diagnostics.push(ConfigurationDiagnostic {
                property_name: key.clone(),
                message: format!("Invalid value: {err}"),
            }),
        }
    }

    let resolved: FormatOptions = serde_json::from_value(serde_json::Value::Object(object))
        .unwrap_or_else(|_| options.clone());

    PluginResolveConfigurationResult {
        file_matching: FileMatchingInfo {
            file_extensions: vec!["ps1".into(), "psm1".into(), "psd1".into()],
            file_names: vec![],
        },
        diagnostics,
        config: resolved,
    }
}

impl SyncPluginHandler<Configuration> for PowerShellPluginHandler {
    fn plugin_info(&mut self) -> PluginInfo {
        PluginInfo {
            name: env!("CARGO_PKG_NAME").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            config_key: "pwsh".to_string(),
            help_url: env!("CARGO_PKG_REPOSITORY").to_string(),
            // Artifacts ship from the release mirror kjanat/dprint-plugin-pwsh
            // (this monorepo stays the source of truth for code), so the
            // canonical public identity is the proxy shorthand kjanat/pwsh:
            // `dprint add kjanat/pwsh`. plugins.dprint.dev serves the
            // mirror's GitHub releases and generates latest.json — checksum
            // included — from the newest one; no hand-built update asset.
            // Tags are bare dash-free semver, equal to the crate version.
            config_schema_url: format!(
                "https://plugins.dprint.dev/kjanat/pwsh/{}/schema.json",
                env!("CARGO_PKG_VERSION")
            ),
            update_url: Some("https://plugins.dprint.dev/kjanat/pwsh/latest.json".to_string()),
        }
    }

    fn license_text(&mut self) -> String {
        // Crate-local copy: a path outside the package root would build in
        // the workspace but break `cargo package`/publish.
        include_str!("../LICENSE").to_string()
    }

    fn resolve_config(
        &mut self,
        config: ConfigKeyMap,
        global_config: &GlobalConfiguration,
    ) -> PluginResolveConfigurationResult<Configuration> {
        resolve_config(config, global_config)
    }

    fn check_config_updates(
        &self,
        _message: CheckConfigUpdatesMessage,
    ) -> Result<Vec<ConfigChange>, FormatError> {
        Ok(Vec::new())
    }

    fn format(
        &mut self,
        request: SyncFormatRequest<Configuration>,
        _format_with_host: impl FnMut(SyncHostFormatRequest) -> FormatResult,
    ) -> FormatResult {
        let source = std::str::from_utf8(&request.file_bytes)
            .map_err(|e| FormatError::new(format!("file is not valid UTF-8: {e}")))?;

        let result = pwsh_formatter::format(source, request.config);
        if !result.formatted {
            // Preserved-input outcome: the formatter appends the diagnostic
            // explaining the skip *after* any parse diagnostics, so report
            // the last one — the first may be an unrelated lex note.
            if let Some(d) = result.diagnostics.last() {
                return Err(FormatError::new(format!(
                    "{}:{}:{} {} [{}]",
                    request.file_path.display(),
                    d.position.line,
                    d.position.column,
                    d.message,
                    d.code.as_str()
                )));
            }
            return Ok(None);
        }
        if result.text == source {
            Ok(None)
        } else {
            Ok(Some(result.text.into_bytes()))
        }
    }
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use dprint_core::generate_plugin_code;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
generate_plugin_code!(PowerShellPluginHandler, PowerShellPluginHandler::new());
