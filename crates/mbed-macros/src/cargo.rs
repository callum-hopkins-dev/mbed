use std::{fmt::Arguments, sync::OnceLock};

use cargo_metadata::{Metadata, MetadataCommand};

#[inline]
pub fn metadata() -> &'static Metadata {
    static METADATA: OnceLock<Metadata> = OnceLock::new();

    METADATA.get_or_init(|| MetadataCommand::new().no_deps().exec().unwrap())
}

#[cfg(any(not(feature = "cargo-progress"), not(target_os = "linux")))]
#[inline]
pub fn emit(_args: Arguments<'_>) {}

#[cfg(all(feature = "cargo-progress", target_os = "linux"))]
pub fn emit(args: Arguments<'_>) {
    use std::io::Write;
    use std::sync::Mutex;
    use std::{fs::File, path::PathBuf, sync::OnceLock};

    static PARENT_STDERR: OnceLock<Option<Mutex<File>>> = OnceLock::new();

    let parent_stderr = PARENT_STDERR.get_or_init(|| {
        std::env::var("CARGO")
            .ok()
            .map(PathBuf::from)
            .and_then(|cargo| {
                let ppid = nix::unistd::getppid();

                std::fs::read_link(format!("/proc/{ppid}/exe"))
                    .is_ok_and(|parent| parent == cargo)
                    .then_some(ppid)
            })
            .and_then(|ppid| {
                File::options()
                    .append(true)
                    .open(format!("/proc/{ppid}/fd/2"))
                    .ok()
            })
            .map(Mutex::new)
    });

    if let Some(parent_stderr) = parent_stderr.as_ref() {
        let mut parent_stderr = parent_stderr.lock().unwrap();

        parent_stderr
            .write_fmt(format_args!("\x1b[2K\r{args}\n"))
            .unwrap();
    }
}
