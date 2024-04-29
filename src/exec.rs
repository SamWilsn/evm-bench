use color_eyre::eyre::{eyre, Result};
use std::path::{Path, PathBuf};

pub fn validate_executable(name: &str, executable: Option<&Path>) -> Result<PathBuf> {
    trace!("validating {name} ({executable:?})");
    match which::which(executable.unwrap_or(name.as_ref())) {
        Ok(executable) => {
            debug!("found {name}: {}", executable.display());
            Ok(executable)
        }
        Err(e) => Err(eyre!("could not find {name}: {e}")),
    }
}
