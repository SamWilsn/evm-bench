use color_eyre::eyre::{eyre, Result};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub fn validate_executable(name: &str, executable: &Path) -> Result<PathBuf> {
    trace!("validating executable {} ({name})", executable.display());
    match Command::new(&executable).arg("--version").output() {
        Ok(out) => {
            debug!(
                "found {name} ({}): {}",
                executable.display(),
                String::from_utf8(out.stdout)
                    .expect("could not decode program stdout")
                    .trim_end_matches("\n")
            );
            Ok(executable.to_path_buf())
        }
        Err(e) => Err(eyre!("{executable:?}: {e}")),
    }
}
