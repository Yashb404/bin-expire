use crate::analyzer::{get_file_times, select_last_used_time};
use crate::models::BinaryInfo;
use walkdir::WalkDir;

pub fn scan_directory(dir: &std::path::Path) -> Vec<BinaryInfo> {
    let mut binaries = Vec::new();

    for entry in WalkDir::new(dir).max_depth(1).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        
        if path.is_dir() {
            continue;
        }

        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let times = get_file_times(path);
        let (last_used, last_used_source) = select_last_used_time(times);
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

        binaries.push(BinaryInfo {
            name,
            path: path.to_path_buf(),
            size: metadata.len(),
            accessed: times.accessed,
            modified: times.modified,
            last_used,
            last_used_source,
            is_symlink: entry.file_type().is_symlink(),
        });
    }

    binaries
}