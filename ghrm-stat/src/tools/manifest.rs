use anyhow::Result;
use std::fs;
use std::path::Path;
use toml::Value;

#[derive(Clone, Debug, Default)]
pub struct Manifest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub license: Option<String>,
    pub dependencies: usize,
    pub kind: Option<String>,
}

pub fn load(root: &Path) -> Result<Manifest> {
    let path = root.join("Cargo.toml");
    if !path.exists() {
        return Ok(Manifest::default());
    }

    let parsed = fs::read_to_string(path)?.parse::<Value>()?;
    let package = parsed.get("package").and_then(Value::as_table);
    let dependencies = parsed
        .get("dependencies")
        .and_then(Value::as_table)
        .map_or(0, toml::map::Map::len);

    Ok(Manifest {
        name: package.and_then(|table| string(table.get("name"))),
        description: package.and_then(|table| string(table.get("description"))),
        version: package.and_then(|table| string(table.get("version"))),
        license: package.and_then(|table| string(table.get("license"))),
        dependencies,
        kind: Some("Cargo".to_string()),
    })
}

fn string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_string)
}
