mod elf_util;
mod mountpoint;
mod extract;
mod mount;

use crate::elf_util::AppImageElf;
use crate::extract::Extractor;
use crate::mountpoint::TempMountPoint;
use clap::Parser;
use std::process::{ExitCode, Command};
use std::env;
use std::os::unix::process::CommandExt;
use std::path::Path;
use log::error;
use subprocess::Exec;
use crate::mount::SquashfuseMounter;

// TODO: rest of the docs
// TODO: insert real version number for --appimage-version to work
#[derive(Parser, Debug)]
// note: we cannot use the default --help, --version, etc., we need to prefix with --appimage-
#[command(version = "0.0.1", disable_version_flag = true, disable_help_flag = true)]
struct Args {
    #[arg(long = "appimage-extract", value_name = "PATTERN", help = "\
        Extract content from embedded filesystem image\n\
        If pattern is passed, only extract matching\n\
        files"
    )]
    extract: Option<Option<String>>,

    #[arg(long = "appimage-extract-and-run", env = "APPIMAGE_EXTRACT_AND_RUN",
        default_value_t = false,
        help = "\
        Temporarily extract content from embedded\n\
        filesystem image, run contained application,\n\
        then delete temporarily extracted content"
    )]
    extract_and_run: bool,

    //#[arg(long = "appimage-help", action = ArgAction::Help, help = "Print this help text", default_value_t = false)]
    //help: bool,

    #[arg(long = "appimage-mount",
        action = clap::ArgAction::SetTrue, default_value_t = false,
        help = "\
        Mount embedded filesystem image and print\n\
        mount point and wait for kill with Ctrl-C"
    )]
    mount: bool,

    #[arg(long = "appimage-offset",
        action = clap::ArgAction::SetTrue, default_value_t = false,
        help = "\
        Print byte offset to start of embedded\n\
        filesystem image"
    )]
    offset: bool,

    #[arg(long = "appimage-portable-home", help = "Create portable home folder to use as $HOME", default_value_t = false)]
    portable_home: bool,

    #[arg(long = "appimage-portable-config", help = "\
        Create a portable config folder to use as\n\
        $XDG_CONFIG_HOME
    ")]
    portable_config: bool,

    #[arg(long = "appimage-signature", help = "Print digital signature embedded in AppImage", default_value_t = false)]
    signature: bool,

    #[arg(long = "appimage-updateinfo", alias = "appimage-updateinformation", help = "Print update info embedded in AppImage", default_value_t = false)]
    update_info: bool,

    #[arg(long = "appimage-version", help = "Print AppImage runtime version")]
    version: bool,
}

fn main() -> ExitCode {
    pretty_env_logger::init();

    // $TARGET_APPIMAGE is a method to launch a "foreign" AppImage with this runtime
    let target_appimage = env::var("TARGET_APPIMAGE");
    let (appimage_path, argv0_path) = if target_appimage.is_ok() {
        let value = target_appimage.unwrap_or_else(|error| {
            todo!()
        });
        (std::path::PathBuf::from(&value), value)
    } else {
        // TODO: properly read argv0 and probably manually /proc/self/exe, as env::current_exe() behavior is underspecified
        let current_exe = env::current_exe().expect("Failed to detect current exe");
        (current_exe.clone().into(), current_exe.as_path().to_str().unwrap().into())
    };

    // create temporary directory as we need one for various reasons
    // note: uses Rust's std::env::temp_dir() which honors $TMPDIR
    // let temp_dir = tempfile::TempDir::with_prefix("appimage-runtime-").unwrap_or_else(|error| {
    //     todo!()
    // });

    let mut elf = AppImageElf::new(appimage_path.clone()).expect("Failed to parse ELF");

    let args = Args::parse();

    let fs_offset = elf.size().unwrap_or_else(|error| {
        todo!()
    });

    if args.offset {
        println!("{}", fs_offset);
        return 0.into();
    }

    if args.update_info {
        println!("{}", &elf.update_information().unwrap_or_default());
        return 0.into();
    }

    if args.signature {
        println!("{}", &elf.signature().unwrap_or_default());
        return 0.into();
    }

    // $VERBOSE support, typically used only when extracting data
    let verbose = env::var("VERBOSE").is_ok();

    if let Some(pattern) = args.extract {
        let out_path = env::current_dir().unwrap().join("squashfs-root");
        let mut extractor = Extractor::new(appimage_path, fs_offset, out_path.into());

        if let Some(pattern) = pattern {
            extractor.set_pattern(pattern);
        }

        extractor.extract(verbose).expect("Extraction failed");
        return 0.into();
    }

    // whether we extract-and-run or mount-and-run, we need a mountpoint
    let temp_mount_point = TempMountPoint::new(&Path::new(&argv0_path).to_path_buf()).expect("Failed to create mountpoint");
    println!("{:?}", temp_mount_point.path());

    if args.extract_and_run {
        let extractor = Extractor::new(appimage_path, fs_offset, temp_mount_point.path().into());
        extractor.extract(verbose).expect("Extraction failed");

        // TODO: forward args
        // TODO: better error handling
        let exit_status = Exec::cmd("./AppRun")
            .cwd(temp_mount_point.path())
            .start()
            .expect("Failed to run AppRun process")
            .wait()
            .expect("Failed to wait for AppRun process");

        let exit_code = exit_status.code().expect("Failed to read AppRun exit code");

        return ExitCode::from(u8::try_from(exit_code).expect("child exit code is outside 0..=255"));
    }

    // now that all parameters and workflows are exhausted, we know that we have to mount the
    // filesystem to our temporary mount point
    let mount_path = temp_mount_point.path().to_path_buf();
    let mounter = SquashfuseMounter::new(temp_mount_point);
    let rt = tokio::runtime::Runtime::new().unwrap();

    // if set, we can just run the filesystem in foreground
    if args.mount {
        if let Err(error) = rt.block_on(mounter.mount(&appimage_path, fs_offset, verbose)) {
            error!("Failed to mount AppImage: {}", error);
            return 1.into();
        }

        // print mount point
        println!("{}", mount_path.to_str().unwrap());

        return 0.into();
    }

    // TODO: run in daemonized process
    if let Err(error) = rt.block_on(mounter.mount(&appimage_path, fs_offset, verbose)) {
        error!("Failed to mount AppImage: {}", error);
        return 1.into();
    }

    // this would never return unless there is an error
    // TODO: while spawn_fuse2 in the mounter creates a proper background
    let err = Command::new("./AppRun")
        .current_dir(mount_path)
        // TODO: args
        .exec();

    error!("Error running AppRun: {}", err);
    1.into()
}
