use std::path::Path;
use std::time::SystemTime;

#[cfg(windows)]
use std::time::{Duration, UNIX_EPOCH};

use crate::models::LastUsedSource;

#[derive(Debug, Clone, Copy)]
pub struct FileTimes {
    pub accessed: Option<SystemTime>,
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Clone, Copy)]
pub struct FileInfo {
    pub size: u64,
    pub times: FileTimes,
}

#[cfg(windows)]
fn filetime_to_systemtime(ft: windows_sys::Win32::Foundation::FILETIME) -> Option<SystemTime> {
    let ticks = ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64);
    if ticks == 0 {
        return None;
    }

    // FILETIME is 100ns ticks since 1601-01-01.
    const WINDOWS_TO_UNIX_EPOCH_TICKS: u64 = 116444736000000000;
    if ticks < WINDOWS_TO_UNIX_EPOCH_TICKS {
        return None;
    }

    let unix_100ns = ticks - WINDOWS_TO_UNIX_EPOCH_TICKS;
    let secs = unix_100ns / 10_000_000;
    let nanos = (unix_100ns % 10_000_000) * 100;
    Some(UNIX_EPOCH + Duration::new(secs, nanos as u32))
}

pub fn get_file_info(file_path: &Path) -> Option<FileInfo> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileAttributesExW, GetFileExInfoStandard, WIN32_FILE_ATTRIBUTE_DATA,
        };

        let wide: Vec<u16> = file_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let mut data: WIN32_FILE_ATTRIBUTE_DATA = unsafe { std::mem::zeroed() };
        let ok = unsafe {
            GetFileAttributesExW(
                wide.as_ptr(),
                GetFileExInfoStandard,
                &mut data as *mut _ as *mut _,
            )
        };

        if ok == 0 {
            return None;
        }

        let size = ((data.nFileSizeHigh as u64) << 32) | (data.nFileSizeLow as u64);

        let accessed = filetime_to_systemtime(data.ftLastAccessTime);
        let modified = filetime_to_systemtime(data.ftLastWriteTime);

        Some(FileInfo {
            size,
            times: FileTimes { accessed, modified },
        })
    }

    #[cfg(not(windows))]
    {
        let metadata = std::fs::metadata(file_path).ok()?;
        let accessed = metadata.accessed().ok();
        let modified = metadata.modified().ok();
        Some(FileInfo {
            size: metadata.len(),
            times: FileTimes { accessed, modified },
        })
    }
}

pub fn select_last_used_time(
    times: FileTimes,
    _windows_use_access_time: bool,
) -> (SystemTime, LastUsedSource) {
    // On Windows, atime can be disabled/delayed and may be updated by scanning.
    // Prefer mtime unless the user explicitly opts into atime.
    #[cfg(windows)]
    {
        if _windows_use_access_time {
            if let Some(accessed) = times.accessed {
                return (accessed, LastUsedSource::Accessed);
            }
        }

        if let Some(modified) = times.modified {
            return (modified, LastUsedSource::Modified);
        }

        if let Some(accessed) = times.accessed {
            return (accessed, LastUsedSource::Accessed);
        }

        (SystemTime::now(), LastUsedSource::Unknown)
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

        (SystemTime::now(), LastUsedSource::Unknown)
    }
}

pub fn is_dormant(timestamp: SystemTime, days_threshold: i64) -> bool {
    let now = SystemTime::now();
    let duration = now.duration_since(timestamp).unwrap_or_default();
    (duration.as_secs() / 86400) as i64 > days_threshold
}

#[cfg(test)]
mod tests {
    use super::*;
    use filetime::{set_file_times, FileTime};
    use std::fs;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[test]
    fn test_get_last_used_time_logic() {
        // 1. Setup: Create a dummy file (unique, in temp dir)
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("bin_expire_unit_{unique}.txt"));
        fs::write(&path, "test data").expect("Failed to create test file");

        // 2. Set the time: We want to simulate a file accessed exactly 10 days ago.
        // 86400 seconds * 10 days = 864000 seconds
        let seconds_in_10_days = 86400 * 10;
        let target_time = SystemTime::now() - Duration::from_secs(seconds_in_10_days);

        // Convert to FileTime for the OS to understand
        let ft = FileTime::from_system_time(target_time);

        // set_file_times(atime, mtime) - We set both to the same old time
        set_file_times(&path, ft, ft).expect("Failed to backdate file");

        // 3. Execute the real production path we care about:
        //    read file attributes -> compute last_used (using mtime here for determinism)
        let info = get_file_info(&path).expect("expected get_file_info to succeed");
        let (result_time, _source) = select_last_used_time(info.times, false);

        // 4. Cleanup
        let _ = fs::remove_file(&path);

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

        println!("Unit Test Passed: Function correctly read the backdated file time.");
    }
}
