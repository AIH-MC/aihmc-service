use std::process::Command;

#[test]
fn help_works_without_runtime_configuration_or_database() {
    let directory = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_aihmc-api"))
        .arg("--help")
        .current_dir(directory.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("--check-config"));
}

#[test]
fn missing_config_and_unknown_arguments_fail_without_disclosing_values() {
    let directory = tempfile::tempdir().unwrap();
    for arguments in [
        vec![],
        vec!["--config"],
        vec!["--unexpected=private-marker"],
        vec!["--config", "CONFIG.EXAMPLE.TOML"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_aihmc-api"))
            .args(arguments)
            .current_dir(directory.path())
            .output()
            .unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("startup failed"));
        assert!(!error.contains("private-marker"));
    }
}
