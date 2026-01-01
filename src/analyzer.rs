use std::fs;
use std::path::Path;
use std::time::SystemTime;

use crate::models::LastUsedSource;

#[derive(Debug, Clone, Copy)]
pub struct FileTimes {
    pub accessed: Option<SystemTime>,
    pub modified: Option<SystemTime>,
}

pub fn get_file_times(file_path: &Path) -> FileTimes {
    // fs::metadata follows symlinks
    let metadata = match fs::metadata(file_path) {
        Ok(meta) => meta,
        Err(_) => {
            return FileTimes {
                accessed: None,
                modified: None,
            }
        }
    };

    FileTimes {
        accessed: metadata.accessed().ok(),
        modified: metadata.modified().ok(),
    }
}

pub fn select_last_used_time(times: FileTimes) -> (SystemTime, LastUsedSource) {
    // Access times are frequently unreliable on Windows (disabled or updated by scanning).
    // Prefer mtime on Windows for deterministic behavior.
    #[cfg(windows)]
    {
        if let Some(modified) = times.modified {
            return (modified, LastUsedSource::Modified);
        }
        if let Some(accessed) = times.accessed {
            return (accessed, LastUsedSource::Accessed);
        }
        return (SystemTime::now(), LastUsedSource::Unknown);
    }

    // On Unix-like systems, use atime when available, fallback to mtime.
    #[cfg(not(windows))]
    {
        if let Some(accessed) = times.accessed {
            return (accessed, LastUsedSource::Accessed);
        }
        if let Some(modified) = times.modified {
            return (modified, LastUsedSource::Modified);
        }
        return (SystemTime::now(), LastUsedSource::Unknown);
    }
}

pub fn get_last_used_time(file_path: &Path) -> SystemTime {
    let (last_used, _) = select_last_used_time(get_file_times(file_path));
    last_used
}

pub fn is_dormant(timestamp: SystemTime, days_threshold: i64) -> bool {
    let now = SystemTime::now();
    let duration = now.duration_since(timestamp).unwrap_or_default();
    (duration.as_secs() / 86400) as i64 > days_threshold
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, Duration};
    use filetime::{FileTime, set_file_times};

    #[test]
    fn test_get_last_used_time_logic() {
        // 1. Setup: Create a dummy file
        let path = Path::new("test_unit_access.txt");
        fs::write(path, "test data").expect("Failed to create test file");

        // 2. Set the time: We want to simulate a file accessed exactly 10 days ago.
        // 86400 seconds * 10 days = 864000 seconds
        let seconds_in_10_days = 86400 * 10;
        let target_time = SystemTime::now() - Duration::from_secs(seconds_in_10_days);
        
        // Convert to FileTime for the OS to understand
        let ft = FileTime::from_system_time(target_time);
        
        // set_file_times(atime, mtime) - We set both to the same old time
        set_file_times(path, ft, ft).expect("Failed to backdate file");

        // 3. Execute the actual function we want to test
        let result_time = get_last_used_time(path);

        // 4. Cleanup
       // fs::remove_file(path).expect("Failed to cleanup test file");

        // 5. Assertion: Calculate the difference between Now and the result
        let now = SystemTime::now();
        let diff = now.duration_since(result_time).unwrap();
        let diff_secs = diff.as_secs();

        // The difference should be roughly 864000 seconds (10 days).
        // We add a buffer of 10 seconds to account for execution time.
        let expected_min = seconds_in_10_days - 10;
        let expected_max = seconds_in_10_days + 10;
        assert!(
            diff_secs >= expected_min && diff_secs <= expected_max,
            "Expected ~10 days ({}s), got {}s",
            seconds_in_10_days,
            diff_secs
        );

        println!("Unit Test Passed: Function correctly read the backdated access time.");
    }
}