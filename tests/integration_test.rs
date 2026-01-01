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
    let config_root = PathBuf::from("test_integration_config");
    let archive_dir = PathBuf::from("test_integration_archive");
    fs::create_dir_all(&test_dir).expect("Failed to create test dir");
    fs::create_dir_all(&archive_dir).expect("Failed to create archive dir");

    // Write a config.toml into BIN_EXPIRE_CONFIG_DIR/bin-expire/config.toml
    let cfg_dir = config_root.join("bin-expire");
    fs::create_dir_all(&cfg_dir).expect("Failed to create config dir");

    // Force deterministic behavior for this test: use mtime rather than atime.
    let archive_str = archive_dir.to_string_lossy().replace('\\', "\\\\");
    let config_toml = format!(
        "ignored_bins = []\ndefault_threshold_days = 90\narchive_path = \"{}\"\nwindows_use_access_time = false\n",
        archive_str
    );
    fs::write(cfg_dir.join("config.toml"), config_toml).expect("Failed to write config.toml");
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
    cmd.env("BIN_EXPIRE_CONFIG_DIR", &config_root);
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
    fs::remove_dir_all(&archive_dir).ok();
    fs::remove_dir_all(&config_root).ok();

    // 6. Assertions
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // We assert that the output contains "STALE"
    assert!(stdout.contains("STALE"), "Output did not contain 'STALE'. Output: \n{}", stdout);
    assert!(stdout.contains("old_tool.exe"), "Output did not contain filename. Output: \n{}", stdout);
    
    println!("Test Passed! Tool correctly detected stale binary.");
}

/// This test verifies that `archive` records a manifest entry and `restore` puts it back.
#[test]
fn test_archive_and_restore_roundtrip() {
    // Setup dirs
    let test_dir = PathBuf::from("test_integration_dir_roundtrip");
    let config_root = PathBuf::from("test_integration_config_roundtrip");
    let archive_dir = PathBuf::from("test_integration_archive_roundtrip");

    fs::create_dir_all(&test_dir).expect("Failed to create test dir");
    fs::create_dir_all(&archive_dir).expect("Failed to create archive dir");

    // Write a config.toml into BIN_EXPIRE_CONFIG_DIR/bin-expire/config.toml
    let cfg_dir = config_root.join("bin-expire");
    fs::create_dir_all(&cfg_dir).expect("Failed to create config dir");

    // Escape backslashes for TOML on Windows
    let archive_str = archive_dir.to_string_lossy().replace('\\', "\\\\");
    let config_toml = format!(
        "ignored_bins = []\ndefault_threshold_days = 90\narchive_path = \"{}\"\nwindows_use_access_time = false\n",
        archive_str
    );
    fs::write(cfg_dir.join("config.toml"), config_toml).expect("Failed to write config.toml");

    // Create a file and backdate it
    let file_name = "old_tool.exe";
    let file_path = test_dir.join(file_name);
    fs::write(&file_path, "content").expect("Failed to write test file");

    let old_time = SystemTime::now() - Duration::from_secs(86400 * 100);
    let ft = FileTime::from_system_time(old_time);
    set_file_times(&file_path, ft, ft).expect("Failed to backdate file");

    // Run archive
    let mut cmd = Command::new("cargo");
    cmd.env("BIN_EXPIRE_CONFIG_DIR", &config_root);
    cmd.args([
        "run",
        "--",
        "archive",
        "-p",
        test_dir.to_str().unwrap(),
        "--days",
        "30",
    ]);
    let output = cmd.output().expect("Failed to execute archive command");
    assert!(output.status.success(), "Archive command failed");

    // It should have moved into archive
    let archived_path = archive_dir.join(file_name);
    assert!(!file_path.exists(), "Original file still exists after archive");
    assert!(archived_path.exists(), "Archived file was not found");

    // Manifest should exist
    let manifest_path = cfg_dir.join("archive.json");
    assert!(manifest_path.exists(), "Manifest archive.json was not created");

    // Run restore
    let mut cmd = Command::new("cargo");
    cmd.env("BIN_EXPIRE_CONFIG_DIR", &config_root);
    cmd.args(["run", "--", "restore", file_name]);
    let output = cmd.output().expect("Failed to execute restore command");
    assert!(output.status.success(), "Restore command failed");

    assert!(file_path.exists(), "File was not restored to original location");

    // Cleanup
    fs::remove_file(&file_path).ok();
    fs::remove_file(&archived_path).ok();
    fs::remove_file(&manifest_path).ok();
    fs::remove_file(cfg_dir.join("config.toml")).ok();
    fs::remove_dir_all(&test_dir).ok();
    fs::remove_dir_all(&archive_dir).ok();
    fs::remove_dir_all(&config_root).ok();
}