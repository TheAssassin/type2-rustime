use std::error::Error;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub(crate) struct TempMountPoint {
    // keeping a reference of TempDir
    // conveniently, it will clean up itself when the variable is dereferenced
    // we just need to keep it alive while in use
    temp_dir: TempDir,
}

impl TempMountPoint {
    pub(crate) fn new(argv0: &PathBuf) -> Result<Self, Box<dyn Error>> {
        let filename = argv0.file_name().unwrap().to_str().unwrap();

        let mut prefix = String::from(".mount_");
        prefix.push_str(&filename[..6.min(filename.len())]);

        let temp_dir = tempfile::Builder::new()
            .prefix(&prefix)
            .tempdir()?;

        Ok(Self { temp_dir })
    }

    pub(crate) fn path(&self) -> &Path {
        self.temp_dir.path()
    }
}
