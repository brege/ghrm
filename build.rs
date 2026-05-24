use std::io::Write;

const ASSET_DIRS: &[&str] = &["css", "img"];
const ASSET_FILES: &[&str] = &["js.sha256.json", "js.tar.zst"];
const ASSET_SCHEMA: &[u8] = b"runtime-assets-v1";

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let assets = std::path::Path::new(&manifest).join("assets");

    println!("cargo:rerun-if-changed=assets");

    let mut files: Vec<std::path::PathBuf> = Vec::new();
    for dir in ASSET_DIRS {
        let path = assets.join(dir);
        println!("cargo:rerun-if-changed={}", path.display());
        collect(&path, &mut files);
    }
    for file in ASSET_FILES {
        let path = assets.join(file);
        println!("cargo:rerun-if-changed={}", path.display());
        files.push(path);
    }
    files.sort();

    let mut hash: u64 = fnv1a(14695981039346656037, ASSET_SCHEMA);
    for path in &files {
        println!("cargo:rerun-if-changed={}", path.display());
        let rel = path.strip_prefix(&assets).unwrap();
        hash = fnv1a(hash, rel.to_string_lossy().as_bytes());
        hash = fnv1a(hash, &std::fs::read(path).unwrap());
    }

    let out = std::env::var("OUT_DIR").unwrap();
    let dest = std::path::Path::new(&out).join("asset_version.txt");
    write!(std::fs::File::create(dest).unwrap(), "{:016x}", hash).unwrap();
}

fn collect(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            println!("cargo:rerun-if-changed={}", path.display());
            collect(&path, out);
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
