//! The browser build has no directory to read Ruby from, so every `.rb` under `ruby/` is put into
//! the binary: `OUT_DIR/ruby_files.rs` is a table of `("ruby/robots/scout.rb", include_str!(…))`.
//! The PC build reads the files themselves (and reloads them when they change); it gets the table
//! too, but does not use it.

use std::path::{Path, PathBuf};

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let mut files = Vec::new();
    collect(&root, &root.join("ruby"), &mut files);
    files.sort();
    let mut out = String::from("pub static RUBY_FILES: &[(&str, &str)] = &[\n");
    for (rel, abs) in &files {
        out.push_str(&format!("    ({rel:?}, include_str!({abs:?})),\n"));
    }
    out.push_str("];\n");
    let dest = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("ruby_files.rs");
    std::fs::write(dest, out).unwrap();
    println!("cargo:rerun-if-changed=ruby");
}

fn collect(root: &Path, dir: &Path, files: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            println!("cargo:rerun-if-changed={}", path.display());
            collect(root, &path, files);
        } else if path.extension().is_some_and(|e| e == "rb") {
            let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            files.push((rel, path.to_string_lossy().into_owned()));
        }
    }
}
