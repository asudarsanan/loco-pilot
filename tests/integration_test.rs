// Integration tests for loco-pilot

/// Test that the binary can execute normally
#[test]
fn test_binary_executes() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_loco-pilot"))
        .output()
        .expect("Failed to execute loco-pilot");
    
    assert!(output.status.success(), "loco-pilot should execute successfully");
}

/// Test that the version command works
#[test]
fn test_version_command() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_loco-pilot"))
        .args(["version"])
        .output()
        .expect("Failed to execute loco-pilot version command");
    
    assert!(output.status.success(), "Version command should execute successfully");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Version:"), "Version output should contain version information");
}

/// Test the config command with no arguments displays current config
#[test]
fn test_config_display() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_loco-pilot"))
        .args(["config"])
        .output()
        .expect("Failed to execute loco-pilot config command");
    
    assert!(output.status.success(), "Config command should execute successfully");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Current configuration:"), "Config output should display current configuration");
}

/// Test different prompt styles
#[test]
fn test_style_options() {
    // Test minimal style
    let minimal_output = std::process::Command::new(env!("CARGO_BIN_EXE_loco-pilot"))
        .args(["--style", "minimal"])
        .output()
        .expect("Failed to execute loco-pilot with minimal style");
    
    assert!(minimal_output.status.success(), "Minimal style should execute successfully");
    
    let minimal_stdout = String::from_utf8_lossy(&minimal_output.stdout);
    assert_eq!(minimal_stdout, "$ ", "Minimal style should be a simple dollar sign and space");
    
    // Test info style has expected components
    let info_output = std::process::Command::new(env!("CARGO_BIN_EXE_loco-pilot"))
        .args(["--style", "info"])
        .output()
        .expect("Failed to execute loco-pilot with info style");
    
    assert!(info_output.status.success(), "Info style should execute successfully");
    
    let info_stdout = String::from_utf8_lossy(&info_output.stdout);
    assert!(info_stdout.contains("["), "Info style should contain time in square brackets");
    assert!(info_stdout.contains("@"), "Info style should contain username@hostname format");
}