use std::fs;
use std::path::Path;
use std::time::SystemTime;

pub fn get_last_used_time(file_path: &Path) -> SystemTime {
    // fs::metadata follows symlinks
    let metadata = match fs::metadata(file_path) {
        Ok(meta) => meta,
        Err(_) => return SystemTime::now(),
    };

    let accessed = metadata.accessed();
    let modified = metadata.modified();

    
    match accessed {
        Ok(acc) => acc,
        Err(_) => modified.unwrap_or(SystemTime::now()),
    }
}

pub fn is_dormant(timestamp: SystemTime, days_threshold: i64) -> bool {
    let now = SystemTime::now();
    let duration = now.duration_since(timestamp).unwrap_or_default();
    (duration.as_secs() / 86400) as i64 > days_threshold
}