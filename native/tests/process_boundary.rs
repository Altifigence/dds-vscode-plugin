// SPDX-FileCopyrightText: 2026 Altifigence
// SPDX-License-Identifier: Apache-2.0
use dds_vscode_integration::transport::run_process;
use std::process::Command;
use std::time::{Duration, Instant};
fn fixture(mode: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fixture-process"));
    command.arg(mode);
    command
}
#[test]
fn real_dds_process_wrapper_has_bounded_io_and_fail_closed_results() {
    assert_eq!(
        run_process(
            fixture("echo"),
            b"fixture-only".to_vec(),
            Duration::from_secs(2),
            1024
        )
        .unwrap(),
        b"fixture-only"
    );
    for mode in ["oversize", "stderr", "fail"] {
        assert!(run_process(fixture(mode), vec![], Duration::from_secs(2), 1024).is_err());
    }
    assert!(run_process(fixture("echo"), vec![0; 2048], Duration::from_secs(2), 1024).is_err());
    let start = Instant::now();
    assert!(run_process(fixture("timeout"), vec![], Duration::from_millis(100), 1024).is_err());
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "deadline must stop the fixture process"
    );
}
