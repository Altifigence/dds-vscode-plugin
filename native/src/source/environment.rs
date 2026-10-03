const SENSITIVE_ENVIRONMENT_KEYS: &[&str] = &[
    "BASH_ENV",
    "CARGO_REGISTRY_TOKEN",
    "DOCKER_AUTH_CONFIG",
    "ENV",
    "GCONV_PATH",
    "GIO_EXTRA_MODULES",
    "GIT_ASKPASS",
    "GI_TYPELIB_PATH",
    "GTK_MODULES",
    "GTK_PATH",
    "HOSTALIASES",
    "KUBECONFIG",
    "LD_AUDIT",
    "LD_DEBUG",
    "LD_DEBUG_OUTPUT",
    "LD_LIBRARY_PATH",
    "LD_PRELOAD",
    "LOCALDOMAIN",
    "LOCPATH",
    "NODE_OPTIONS",
    "NODE_PATH",
    "NPM_TOKEN",
    "PERL5LIB",
    "PERL5OPT",
    "PYTHONHOME",
    "PYTHONINSPECT",
    "PYTHONPATH",
    "PYTHONSTARTUP",
    "QT_PLUGIN_PATH",
    "QT_QPA_PLATFORM_PLUGIN_PATH",
    "RUBYOPT",
    "RUST_BACKTRACE",
    "RUST_LOG",
    "SSH_AGENT_PID",
    "SSH_AUTH_SOCK",
    "SSL_CERT_DIR",
    "SSL_CERT_FILE",
    "SUDO_ASKPASS",
];

const SENSITIVE_ENVIRONMENT_PREFIXES: &[&str] = &[
    "ALTIF_",
    "ANTHROPIC_",
    "AWS_",
    "AZURE_",
    "DYLD_",
    "GITHUB_",
    "GITLAB_",
    "GOOGLE_",
    "OPENAI_",
];
/// Removes inherited loader, interpreter, and credential variables before spawn.
///
/// This is defense in depth. It does not replace an OS process sandbox or a
/// credential broker with an independently enforced access policy.
pub fn sanitize_child_environment(command: &mut Command) {
    let explicit_keys = command
        .get_envs()
        .map(|(key, _)| key.to_os_string())
        .collect::<Vec<_>>();
    for key in SENSITIVE_ENVIRONMENT_KEYS {
        command.env_remove(key);
    }
    for key in std::env::vars_os().map(|(key, _)| key).chain(explicit_keys) {
        if is_sensitive_environment_key(&key) {
            command.env_remove(key);
        }
    }
}
fn is_sensitive_environment_key(key: &OsStr) -> bool {
    let key = key.to_string_lossy().to_ascii_uppercase();
    SENSITIVE_ENVIRONMENT_KEYS.contains(&key.as_str())
        || SENSITIVE_ENVIRONMENT_PREFIXES
            .iter()
            .any(|prefix| key.starts_with(prefix))
}
