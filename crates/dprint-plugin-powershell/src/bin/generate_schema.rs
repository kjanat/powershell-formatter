//! Generates `deployment/schema.json` from the [`FormatOptions`] type so the
//! published schema can never drift from the core configuration model.
//!
//! Run: `cargo run -p dprint-plugin-powershell --features schema --bin generate-schema`

use powershell_formatter::FormatOptions;

fn sort_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<(String, serde_json::Value)> =
                map.into_iter().map(|(k, v)| (k, sort_value(v))).collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            serde_json::Value::Object(entries.into_iter().collect())
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(sort_value).collect())
        }
        other => other,
    }
}

fn main() {
    let schema = schemars::schema_for!(FormatOptions);
    let mut value = serde_json::to_value(&schema).expect("schema serializes");
    if let serde_json::Value::Object(map) = &mut value {
        map.insert(
            "$id".to_owned(),
            serde_json::Value::String(format!(
                "https://plugins.dprint.dev/kjanat/powershell/{}/schema.json",
                env!("CARGO_PKG_VERSION")
            )),
        );
        map.insert(
            "title".to_owned(),
            serde_json::Value::String("PowerShell formatter configuration".to_owned()),
        );
    }
    let sorted = sort_value(value);
    let out = serde_json::to_string_pretty(&sorted).expect("stable json") + "\n";
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("deployment");
    std::fs::create_dir_all(&dir).expect("mkdir deployment");
    std::fs::write(dir.join("schema.json"), out).expect("write schema.json");
    println!("wrote {}", dir.join("schema.json").display());
}
