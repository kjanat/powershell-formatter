//! Generates `deployment/schema.json` from the [`FormatOptions`] type so the
//! published schema can never drift from the core configuration model.
//!
//! Run: `cargo run -p dprint-plugin-powershell --features schema --bin generate-schema`

use powershell_formatter::FormatOptions;

fn main() {
    let schema = schemars::schema_for!(FormatOptions);
    let mut value = serde_json::to_value(&schema).expect("schema serializes");
    if let serde_json::Value::Object(map) = &mut value {
        map.insert(
            "$id".to_owned(),
            serde_json::Value::String(format!(
                // Full repo name: plugins.dprint.dev serves any GitHub
                // repo's releases; only repos actually named
                // dprint-plugin-<x> may shorten the path.
                "https://plugins.dprint.dev/kjanat/powershell-formatter/{}/schema.json",
                env!("CARGO_PKG_VERSION")
            )),
        );
        map.insert(
            "title".to_owned(),
            serde_json::Value::String("PowerShell formatter configuration".to_owned()),
        );
    }
    // The same schema-aware canonical order the dprint `json-schema-sort`
    // plugin applies, so the committed file never fights `dprint fmt`.
    let sorted = json_schema_sort::sorted_schema(value);
    let out = serde_json::to_string_pretty(&sorted).expect("stable json") + "\n";
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("deployment");
    std::fs::create_dir_all(&dir).expect("mkdir deployment");
    std::fs::write(dir.join("schema.json"), out).expect("write schema.json");
    println!("wrote {}", dir.join("schema.json").display());
}
