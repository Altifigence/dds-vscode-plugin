pub fn run_process(
    mut command: Command,
    input: Vec<u8>,
    timeout: Duration,
    limit: usize,
) -> Result<Vec<u8>, String> {
    if input.len() > limit {
        return Err("Linux backend request exceeds its byte limit.".into());
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("WSL backend could not start: {e}"))?;
    enum Event {
        Data(bool, Vec<u8>),
        Done,
        Error,
    }
    let (sender, receiver) = mpsc::sync_channel(8);
    for (stdout, mut pipe) in [
        (
            true,
            Box::new(child.stdout.take().unwrap()) as Box<dyn Read + Send>,
        ),
        (
            false,
            Box::new(child.stderr.take().unwrap()) as Box<dyn Read + Send>,
        ),
    ] {
        let sender = sender.clone();
        std::thread::spawn(move || {
            let mut buffer = [0; 8192];
            loop {
                match pipe.read(&mut buffer) {
                    Ok(0) => {
                        let _ = sender.send(Event::Done);
                        break;
                    }
                    Ok(size) => {
                        if sender
                            .send(Event::Data(stdout, buffer[..size].to_vec()))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(_) => {
                        let _ = sender.send(Event::Error);
                        break;
                    }
                }
            }
        });
    }
    let mut stdin = child.stdin.take().unwrap();
    std::thread::spawn(move || {
        if stdin.write_all(&input).is_err() {
            let _ = sender.send(Event::Error);
        }
    });
    let deadline = Instant::now() + timeout;
    let mut output = Vec::new();
    let mut error = Vec::new();
    let mut done = 0;
    let result = loop {
        if Instant::now() >= deadline {
            break Err(
                "WSL backend timed out. Reopen the project after checking the distribution.".into(),
            );
        }
        match receiver.recv_timeout(Duration::from_millis(20)) {
            Ok(Event::Data(stdout, bytes)) => {
                if output.len() + error.len() + bytes.len() > limit {
                    break Err("Linux backend response exceeds its byte limit.".into());
                }
                if stdout {
                    output.extend(bytes);
                } else {
                    error.extend(bytes);
                }
            }
            Ok(Event::Done) => done += 1,
            Ok(Event::Error) => break Err("WSL backend communication failed.".into()),
            Err(mpsc::RecvTimeoutError::Disconnected) | Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        match child.try_wait() {
            Ok(Some(status)) if done == 2 => {
                if !status.success() {
                    let message = serde_json::from_slice::<Value>(&error)
                        .ok()
                        .and_then(|v| v.get("message").and_then(Value::as_str).map(str::to_owned));
                    break Err(message.unwrap_or_else(|| "WSL backend failed. Verify that the matching DDS Linux backend is installed, then reopen the project.".into()));
                }
                if !error.is_empty() {
                    break Err("Linux backend returned unexpected error output.".into());
                }
                break Ok(output);
            }
            Err(_) => break Err("WSL backend process state is unavailable.".into()),
            _ => {}
        }
    };
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}
