use crate::{Context, Row, repo};
use anyhow::Result;

pub fn run(ctx: &Context) -> Result<Vec<Row>> {
    let index = repo(ctx).index()?;
    let bytes = index
        .entries()
        .iter()
        .map(|entry| entry.stat.size as u64)
        .sum::<u64>();
    let files = index.entries().len();

    Ok(vec![
        Row::new("bytes", bytes.to_string()),
        Row::new("size", format_bytes(bytes)),
        Row::new("files", files.to_string()),
    ])
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_format_without_unit_jump() {
        assert_eq!(format_bytes(1023), "1023 B");
    }

    #[test]
    fn bytes_format_binary_units() {
        assert_eq!(format_bytes(1024), "1.00 KiB");
    }
}
