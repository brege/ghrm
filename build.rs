use std::io::Write;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let assets = std::path::Path::new(&manifest).join("assets");

    println!("cargo:rerun-if-changed=assets");

    let mut files: Vec<std::path::PathBuf> = Vec::new();
    collect(&assets, &assets, &mut files);
    files.sort();

    let mut hash: u64 = 14695981039346656037;
    for path in &files {
        let rel = path.strip_prefix(&assets).unwrap();
        hash = fnv1a(hash, rel.to_string_lossy().as_bytes());
        hash = fnv1a(hash, &std::fs::read(path).unwrap());
    }

    let out = std::env::var("OUT_DIR").unwrap();
    let dest = std::path::Path::new(&out).join("theme_version.txt");
    write!(std::fs::File::create(dest).unwrap(), "{:016x}", hash).unwrap();
}

fn collect(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap().to_owned();
        if rel.starts_with("vendor") || rel == std::path::Path::new("config.json") {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, out);
        } else {
            out.push(path);
        }
    }
}

fn fnv1a(mut h: u64, data: &[u8]) -> u64 {
    for b in data {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}
