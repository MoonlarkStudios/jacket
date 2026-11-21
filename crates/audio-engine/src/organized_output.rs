use crate::{EngineError, Result};
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

/// The exact extensionless destination admitted for an organized audio file.
/// Its root must already exist and can never be created by the exporter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrganizedOutput {
    root: PathBuf,
    stem: PathBuf,
}

impl OrganizedOutput {
    pub fn new(root: PathBuf, stem: PathBuf) -> Result<Self> {
        let relative = stem.strip_prefix(&root).ok();
        if !root.is_absolute()
            || relative.is_none_or(|path| {
                path.file_name().is_none()
                    || path
                        .components()
                        .any(|part| !matches!(part, Component::Normal(_)))
            })
        {
            return Err(EngineError::Invalid(
                "Choose a filename inside the music folder.".into(),
            ));
        }
        Ok(Self { root, stem })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn stem(&self) -> &Path {
        &self.stem
    }

    pub fn check(&self) -> Result<()> {
        self.check_parent(false)
    }

    pub(crate) fn prepare_parent(&self) -> Result<()> {
        self.check_parent(true)
    }

    fn check_parent(&self, create: bool) -> Result<()> {
        if !self.root.is_dir() || !self.root.canonicalize().is_ok_and(|path| path == self.root) {
            return Err(EngineError::Invalid(
                "The saved music folder is unavailable. Reconnect it, choose another folder, or use Save As…".into(),
            ));
        }
        let relative = self
            .stem
            .strip_prefix(&self.root)
            .expect("validated organized path");
        let parent = relative.parent().expect("organized path has a filename");
        let mut current = self.root.clone();
        for component in parent.components() {
            current.push(component);
            let metadata = match current.symlink_metadata() {
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    if !create {
                        return Ok(());
                    }
                    match fs::create_dir(&current) {
                        Ok(()) => (),
                        Err(error) if error.kind() == ErrorKind::AlreadyExists => (),
                        Err(error) => return Err(error.into()),
                    }
                    current.symlink_metadata()?
                }
                result => result?,
            };
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(EngineError::Invalid(format!(
                    "Can’t save inside {}. Choose another music folder, change the rule, or use Save As…",
                    current.display(),
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checking_is_read_only_and_a_missing_root_is_never_created() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap().join("Music");
        let output = OrganizedOutput::new(root.clone(), root.join("Artist/Album/Song")).unwrap();
        assert!(output.prepare_parent().is_err());
        assert!(!root.exists());
        fs::create_dir(&root).unwrap();
        output.check().unwrap();
        assert!(!root.join("Artist").exists());
        output.prepare_parent().unwrap();
        assert!(root.join("Artist/Album").is_dir());
        assert!(!output.stem().exists());
    }

    #[test]
    fn reject_escape_paths_and_file_blockers() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        assert!(OrganizedOutput::new(root.clone(), root.join("Artist/../../Song")).is_err());
        assert!(OrganizedOutput::new(root.clone(), root.with_extension("other")).is_err());
        fs::write(root.join("Artist"), b"existing file").unwrap();
        let output = OrganizedOutput::new(root.clone(), root.join("Artist/Album/Song")).unwrap();
        assert!(output.check().is_err());
        assert!(output.prepare_parent().is_err());
        assert_eq!(fs::read(root.join("Artist")).unwrap(), b"existing file");
    }

    #[cfg(unix)]
    #[test]
    fn an_existing_or_replaced_directory_link_cannot_redirect_the_export() {
        let directory = tempfile::tempdir().unwrap();
        let fixture = directory.path().canonicalize().unwrap();
        let root = fixture.join("Music");
        let other = fixture.join("Other");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&other).unwrap();
        let output = OrganizedOutput::new(root.clone(), root.join("Artist/Album/Song")).unwrap();
        output.check().unwrap();
        std::os::unix::fs::symlink(&other, root.join("Artist")).unwrap();
        assert!(output.prepare_parent().is_err());
        assert!(!other.join("Album").exists());
        fs::remove_file(root.join("Artist")).unwrap();
        fs::remove_dir(&root).unwrap();
        std::os::unix::fs::symlink(&other, &root).unwrap();
        assert!(output.check().is_err());
    }
}
