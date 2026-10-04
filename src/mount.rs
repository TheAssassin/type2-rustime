use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use crate::mountpoint::TempMountPoint;
use fuser_async::FilesystemFUSE;
use squashfs_async::{pools, Options, SquashFs};

pub(crate) struct SquashfuseMounter {
    temp_mount_point: TempMountPoint,
}

impl SquashfuseMounter {
    pub(crate) fn new(temp_mount_point: TempMountPoint) -> Self {
        SquashfuseMounter { temp_mount_point }
    }

    pub(crate) async fn mount(self, appimage_path: &PathBuf, fs_offset: u64, verbose: bool) -> Result<(), Box<dyn Error>> {
        let reader = BufReader::new(File::open(&appimage_path)?);

        // let fs = squashfuse_rs::SquashfsFilesystem::new(
        //     backhand::FilesystemReader::from_reader_with_offset(reader, fs_offset)?,
        //     verbose
        // );
        // let mount_options = vec![
        //     fuser::MountOption::FSName("squashfuse".to_string()),
        //     fuser::MountOption::RO,
        // ];
        //
        // if let Err(error) = fuser::mount2(fs, self.temp_mount_point.path(), &mount_options) {
        //     return Err(error.into());
        // }

        let options = Options {
            cache_mb: 100,
            readers: 4,
            direct_limit: 0,
            fs_offset,
        };

        let fs = SquashFs::<pools::LocalReadersPoolTokio>::open(appimage_path, &options).await?;

        let fuse = FilesystemFUSE::new(fs);

        let _mount = fuser::spawn_mount2(
            fuse,
            self.temp_mount_point.path(),
            &[fuser::MountOption::RO, fuser::MountOption::Async],
        )?;
        tokio::signal::ctrl_c().await?;
        Ok(())
    }
}
