use std::process::Command;

#[test]
fn help_is_a_successful_discovery_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("--help")
        .output()
        .expect("run xtask --help");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("verify-bindings"));
}

#[test]
fn unknown_commands_fail_for_automation() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("definitely-not-a-command")
        .output()
        .expect("run xtask with an unknown command");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown xtask command"));
}

#[test]
fn missing_command_fails_without_running_a_default_mutation() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .output()
        .expect("run xtask without a command");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("explicit xtask command"));
}

#[test]
fn known_commands_reject_unconsumed_arguments() {
    for arguments in [
        &["help", "extra"][..],
        &["--help", "extra"][..],
        &["build-cimgui-provider", "extra"][..],
        &["web-demo", "implot", "extra"][..],
        &["verify-bindings", "--check-only", "--check-only"][..],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(arguments)
            .output()
            .expect("run xtask with extra arguments");

        assert!(
            !output.status.success(),
            "command unexpectedly accepted arguments: {arguments:?}"
        );
    }
}
