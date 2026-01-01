use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, Duration};
use filetime::{FileTime, set_file_times};

/// This test verifies that the tool correctly identifies a fake old binary.
#[test]
fn test_detects_stale_binary() {
    // 1. Setup: Create a temporary directory
    let test_dir = PathBuf::from("test_integration_dir");
    fs::create_dir_all(&test_dir).expect("Failed to create test dir");
    let file_path = test_dir.join("old_tool.exe");

    // 2. Create the file
    fs::write(&file_path, "content").expect("Failed to write test file");

    // 3. Backdate the file to 100 days ago
    let old_time = SystemTime::now() - Duration::from_secs(86400 * 100);
    let ft = FileTime::from_system_time(old_time);
    set_file_times(&file_path, ft, ft).expect("Failed to backdate file");

    // 4. Execute the binary (This runs 'cargo run' logic)
    // We invoke the debug build directly
    let mut cmd = Command::new("cargo");
    cmd.args([
        "run",
        "--",
        "scan",
        "-p", test_dir.to_str().unwrap(),
        "--days", "30"
    ]);

    let output = cmd.output().expect("Failed to execute command");

    // 5. Cleanup
    fs::remove_file(&file_path).ok();
    fs::remove_dir(&test_dir).ok();

    // 6. Assertions
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // We assert that the output contains "STALE"
    assert!(stdout.contains("STALE"), "Output did not contain 'STALE'. Output: \n{}", stdout);
    assert!(stdout.contains("old_tool.exe"), "Output did not contain filename. Output: \n{}", stdout);
    
    println!("Test Passed! Tool correctly detected stale binary.");
}