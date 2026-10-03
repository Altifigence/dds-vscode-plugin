#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WslProject {
    pub distribution: String,
    pub directory: String,
}

impl WslProject {
    pub fn new(distribution: String, directory: String) -> Result<Self, String> {
        if distribution.is_empty()
            || distribution.len() > 64
            || distribution.starts_with('-')
            || !distribution
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
        {
            return Err("Select a valid WSL distribution.".into());
        }
        crate::validate_project(&directory)?;
        Ok(Self {
            distribution,
            directory,
        })
    }

    /// Only the native folder picker/startup path calls this, never a renderer path.
    pub fn from_unc(path: &str) -> Result<Option<Self>, String> {
        let path = path
            .strip_prefix(r"\\?\UNC\")
            .map(|s| format!(r"\\{s}"))
            .unwrap_or_else(|| path.into());
        let Some(unc) = path.strip_prefix(r"\\") else {
            return Ok(None);
        };
        let Some((server, rest)) = unc.split_once('\\') else {
            return Ok(None);
        };
        if !server.eq_ignore_ascii_case("wsl.localhost") && !server.eq_ignore_ascii_case("wsl$") {
            return Ok(None);
        }
        let (distribution, directory) = rest
            .split_once('\\')
            .ok_or("Select a project inside the WSL distribution.")?;
        let directory = format!("/{}", directory.replace('\\', "/"));
        Ok(Some(Self::new(distribution.into(), directory)?))
    }
}
