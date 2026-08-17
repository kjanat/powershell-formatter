//! Plugin behavior tests driven through the `SyncPluginHandler` (no Wasm
//! host needed); the true end-to-end check against the real dprint binary
//! lives in `tests/e2e.sh`.

use dprint_core::configuration::{ConfigKeyMap, ConfigKeyValue, GlobalConfiguration};
use dprint_core::plugins::{
    FileMatchingInfo, NullCancellationToken, SyncFormatRequest, SyncPluginHandler,
};
use dprint_plugin_pwsh::{PowerShellPluginHandler, resolve_config};
use std::path::Path;

fn format_text(config_entries: &[(&str, ConfigKeyValue)], source: &str) -> Option<String> {
    let mut handler = PowerShellPluginHandler::new();
    let mut config = ConfigKeyMap::new();
    for (k, v) in config_entries {
        config.insert((*k).to_string(), v.clone());
    }
    let resolved = resolve_config(config, &GlobalConfiguration::default());
    assert!(
        resolved.diagnostics.is_empty(),
        "diagnostics: {:?}",
        resolved.diagnostics
    );
    let token = NullCancellationToken;
    let result = handler
        .format(
            SyncFormatRequest {
                file_path: Path::new("test.ps1"),
                config_id: dprint_core::plugins::FormatConfigId::from_raw(1),
                file_bytes: source.as_bytes().to_vec(),
                config: &resolved.config,
                range: None,
                token: &token,
            },
            |_| Ok(None),
        )
        .expect("format succeeds");
    result.map(|bytes| String::from_utf8(bytes).expect("utf8"))
}

#[test]
fn formats_and_is_idempotent() {
    let out = format_text(&[], "function foo {\n\"hello\"\n  }").expect("changes");
    assert_eq!(out, "function foo {\n    \"hello\"\n}");
    // Idempotence: canonical input returns None so `dprint check` passes.
    assert_eq!(format_text(&[], &out), None);
}

#[test]
fn respects_config_keys() {
    let out = format_text(
        &[("indentWidth", ConfigKeyValue::Number(2))],
        "if ($x) {\n1\n}",
    )
    .expect("changes");
    assert_eq!(out, "if ($x) {\n  1\n}");

    let out = format_text(
        &[("braceStyle", ConfigKeyValue::String("nextLine".to_string()))],
        "if ($x) {\n1\n}",
    )
    .expect("changes");
    assert_eq!(out, "if ($x)\n{\n    1\n}");
}

#[test]
fn unknown_key_yields_diagnostic() {
    let mut config = ConfigKeyMap::new();
    config.insert("frobnicate".to_string(), ConfigKeyValue::Bool(true));
    let resolved = resolve_config(config, &GlobalConfiguration::default());
    assert_eq!(resolved.diagnostics.len(), 1);
    assert_eq!(resolved.diagnostics[0].property_name, "frobnicate");
}

#[test]
fn invalid_value_yields_diagnostic_and_keeps_default() {
    let mut config = ConfigKeyMap::new();
    config.insert(
        "braceStyle".to_string(),
        ConfigKeyValue::String("sideways".to_string()),
    );
    let resolved = resolve_config(config, &GlobalConfiguration::default());
    assert_eq!(resolved.diagnostics.len(), 1);
    assert_eq!(resolved.diagnostics[0].property_name, "braceStyle");
    assert_eq!(
        resolved.config.brace_style,
        pwsh_formatter::BraceStyle::SameLine
    );
}

#[test]
fn inherits_global_configuration() {
    let global = GlobalConfiguration {
        indent_width: Some(2),
        use_tabs: Some(false),
        line_width: Some(80),
        new_line_kind: None,
    };
    let resolved = resolve_config(ConfigKeyMap::new(), &global);
    assert_eq!(resolved.config.indent_width, 2);
    assert_eq!(resolved.config.line_width, 80);
}

#[test]
fn invalid_utf8_is_an_error_not_a_panic() {
    let mut handler = PowerShellPluginHandler::new();
    let resolved = resolve_config(ConfigKeyMap::new(), &GlobalConfiguration::default());
    let token = NullCancellationToken;
    let result = handler.format(
        SyncFormatRequest {
            file_path: Path::new("bad.ps1"),
            config_id: dprint_core::plugins::FormatConfigId::from_raw(1),
            file_bytes: vec![0xFF, 0xFE, 0x00],
            config: &resolved.config,
            range: None,
            token: &token,
        },
        |_| Ok(None),
    );
    assert!(result.is_err());
}

#[test]
fn malformed_powershell_is_an_error_with_location() {
    let mut handler = PowerShellPluginHandler::new();
    let resolved = resolve_config(ConfigKeyMap::new(), &GlobalConfiguration::default());
    let token = NullCancellationToken;
    let result = handler.format(
        SyncFormatRequest {
            file_path: Path::new("broken.ps1"),
            config_id: dprint_core::plugins::FormatConfigId::from_raw(1),
            file_bytes: b"function f {\n 'x'\n".to_vec(),
            config: &resolved.config,
            range: None,
            token: &token,
        },
        |_| Ok(None),
    );
    let err = result.expect_err("must error").to_string();
    assert!(err.contains("broken.ps1"), "err: {err}");
}

#[test]
fn plugin_info_sanity() {
    let mut handler = PowerShellPluginHandler::new();
    let info = handler.plugin_info();
    assert_eq!(info.config_key, "pwsh");
    assert!(info.config_schema_url.ends_with("/schema.json"));
    assert!(info.update_url.is_some());
    let matching: FileMatchingInfo =
        resolve_config(ConfigKeyMap::new(), &GlobalConfiguration::default()).file_matching;
    assert!(matching.file_extensions.contains(&"ps1".to_string()));
}
