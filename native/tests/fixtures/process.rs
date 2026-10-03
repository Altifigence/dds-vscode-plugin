// SPDX-FileCopyrightText: 2026 Altifigence
// SPDX-License-Identifier: Apache-2.0
// Test fixture only: never discovers or opens a real VS Code application.
use std::io::{Read, Write};
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--locate-extension") {
        let mode = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .join("fixture-extension-mode");
        match std::fs::read_to_string(mode).as_deref() {
            Ok("missing") => {}
            Ok("invalid") => println!("relative/extension"),
            Ok("oversize") => std::io::stdout().write_all(&[b'x'; 16 * 1024 + 1]).unwrap(),
            _ => println!("C:\\DDS-Fixture\\ms-vscode-remote.remote-wsl"),
        }
        return;
    }
    match args.first().map(String::as_str) {
        Some("echo") => {
            let mut input = Vec::new();
            std::io::stdin().read_to_end(&mut input).unwrap();
            std::io::stdout().write_all(&input).unwrap();
        }
        Some("oversize") => std::io::stdout().write_all(&[b'x'; 8192]).unwrap(),
        Some("stderr") => eprintln!("fixture error output"),
        Some("fail") => std::process::exit(7),
        Some("timeout") => std::thread::sleep(std::time::Duration::from_secs(3)),
        Some("environment") => {
            println!(
                "{}",
                std::env::var("OPENAI_FIXTURE_KEY").unwrap_or_else(|_| "removed".into())
            );
        }
        _ => {}
    }
}
