use std::fs;
use std::io;
use std::path::{Component, Path};

use anyhow::{Context, Result, bail};

pub fn all(resource_dir: &Path, output_dir: &Path) -> Result<()> {
    validate_target(resource_dir)?;
    validate_target(output_dir)?;

    remove_target("resources", resource_dir)?;
    if output_dir != resource_dir {
        remove_target("output", output_dir)?;
    }
    Ok(())
}

fn validate_target(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        bail!("refusing to clean an empty path");
    }

    let mut normal_components = 0;
    for component in path.components() {
        match component {
            Component::Normal(_) => normal_components += 1,
            Component::ParentDir => {
                bail!("refusing to clean path containing '..': {}", path.display())
            }
            Component::Prefix(_) | Component::RootDir | Component::CurDir => {}
        }
    }
    if normal_components == 0 {
        bail!("refusing to clean dangerous path {}", path.display());
    }
    Ok(())
}

fn remove_target(label: &str, path: &Path) -> Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            println!("No {label} to remove at {}", path.display());
            return Ok(());
        }
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to inspect {}", path.display()));
        }
    };

    if metadata.file_type().is_symlink() {
        fs::remove_file(path)
            .with_context(|| format!("failed to remove symlink {}", path.display()))?;
    } else if metadata.is_dir() {
        fs::remove_dir_all(path)
            .with_context(|| format!("failed to remove directory {}", path.display()))?;
    } else {
        bail!("refusing to clean non-directory path {}", path.display());
    }
    println!("Removed {label} at {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn removes_resource_and_output_directories() {
        let temp = TempDir::new().unwrap();
        let resources = temp.path().join("resources");
        let output = temp.path().join("output");
        fs::create_dir_all(&resources).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(resources.join("data"), "data").unwrap();
        fs::write(output.join("card.png"), "png").unwrap();

        all(&resources, &output).unwrap();

        assert!(!resources.exists());
        assert!(!output.exists());
    }

    #[test]
    fn handles_identical_targets_once() {
        let temp = TempDir::new().unwrap();
        let target = temp.path().join("shared");
        fs::create_dir_all(&target).unwrap();

        all(&target, &target).unwrap();

        assert!(!target.exists());
    }

    #[test]
    fn missing_targets_are_already_clean() {
        let temp = TempDir::new().unwrap();

        all(&temp.path().join("resources"), &temp.path().join("output")).unwrap();
    }

    #[test]
    fn rejects_dangerous_paths_before_removing_anything() {
        let temp = TempDir::new().unwrap();
        let resources = temp.path().join("resources");
        fs::create_dir_all(&resources).unwrap();

        assert!(all(&resources, Path::new(".")).is_err());
        assert!(resources.exists());
        assert!(all(Path::new("../resources"), Path::new("output")).is_err());
        assert!(all(Path::new("/"), Path::new("output")).is_err());
    }
}
