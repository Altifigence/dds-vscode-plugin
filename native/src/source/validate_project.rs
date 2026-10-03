/// Paths are Linux paths even when validated by the Windows client.
pub fn validate_project(project: &str) -> Result<(), String> {
    if !project.starts_with('/')
        || project.len() > 4096
        || project == "/"
        || project.chars().any(|c| c.is_control() || c == '\\')
        || project[1..]
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err("Select an absolute Linux project directory without aliases.".into());
    }
    Ok(())
}
