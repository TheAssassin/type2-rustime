use std::error::Error;
use std::{fs, io};
use std::fs::{File, Permissions};
use std::io::BufReader;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::PathBuf;
use backhand::{FilesystemReader, InnerNode};
use log::{info, warn};

#[derive(Default, Debug)]
pub(crate) struct Extractor {
    appimage_path: PathBuf,
    offset: u64,
    out_dir_path: PathBuf,
    pattern: Option<String>,
}

impl Extractor {
    pub(crate) fn new(appimage_path: PathBuf, offset: u64, out_dir_path: PathBuf) -> Extractor {
        Extractor { appimage_path, out_dir_path, offset, ..Default::default() }
    }

    pub(crate) fn set_pattern(&mut self, pattern: String) {
        self.pattern = Some(pattern);
    }

    // TODO: handle existing files (ideally, use some filesize comparison or similar to speed up)
    pub(crate) fn extract(&self, verbose: bool) -> Result<(), Box<dyn Error>> {
        let reader = BufReader::new(File::open(&self.appimage_path)?);

        let fs = FilesystemReader::from_reader_with_offset(reader, self.offset)?;

        // first all, create out path directory
        fs::create_dir_all(&self.out_dir_path)?;

        for node in fs.files() {
            if verbose {
                info!("{}", node.fullpath.as_path().to_str().expect("Conversion to string failed"));
            }

            // using strip_prefix to strip leading / if needed, otherwise use path as-is
            let out_path = self.out_dir_path.join(&node.fullpath.strip_prefix("/").unwrap_or(&node.fullpath));

            let perms = Permissions::from_mode(node.header.permissions as u32);

            if self.pattern.is_some() {
                todo!()
            }

            match &node.inner {
                InnerNode::Dir(_) => {
                    fs::create_dir_all(&out_path)?;

                    fs::set_permissions(&out_path, perms)?;
                }

                InnerNode::File(file_meta) => {
                    // create parent directories (if set)
                    if let Some(parent) = out_path.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    let mut reader = fs.file(&file_meta.basic).reader();

                    let mut out_file = File::create(&out_path)?;
                    io::copy(&mut reader, &mut out_file)?;

                    fs::set_permissions(&out_path, perms)?;
                }

                InnerNode::Symlink(link) => {
                    if let Some(parent) = out_path.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    symlink(&link.link, &out_path)?;
                }

                _ => {
                    warn!("Skipping special/unkown file type: {:?}", node.fullpath);
                }
            }
        }

        Ok(())
    }

}
