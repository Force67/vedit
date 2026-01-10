use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

pub fn compute_fingerprint(path: &str) -> u64 {
    let resolved = canonicalize_lossy(path);
    let mut hasher = DefaultHasher::new();
    resolved.hash(&mut hasher);
    hasher.finish()
}

fn canonicalize_lossy(path: &str) -> String {
    let path_buf = PathBuf::from(path);
    fs::canonicalize(&path_buf)
        .unwrap_or(path_buf)
        .to_string_lossy()
        .to_string()
}
