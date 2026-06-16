use std::{borrow::Cow::Borrowed, fs::File, ops::Deref};

use mbed_macros::{Artifact, LocalFile, Writer};
use tanager::Parse;

#[proc_macro]
pub fn include_bytes(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    #[derive(Debug, Hash, Parse)]
    #[tanager(transparent)]
    struct Input(Files);

    impl std::fmt::Display for Input {
        #[inline]
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "mbed::include_bytes {}", self.0)
        }
    }

    mbed_macros::execute_fn(item.into(), |deps, input: Input| {
        deps.track_files(&*input.0);

        let mut writer = Writer::new()?;

        for path in input.0.iter() {
            std::io::copy(&mut File::open(path)?, &mut writer)?;
        }

        std::io::Result::Ok(Artifact {
            mime: input
                .0
                .first()
                .map(|x| {
                    mime_guess::from_path(x)
                        .first_or_octet_stream()
                        .to_string()
                        .into()
                })
                .unwrap_or(Borrowed("application/octet-stream")),

            ..writer.finish()?
        })
    })
    .into()
}

#[proc_macro]
pub fn include_str(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    #[derive(Debug, Hash, Parse)]
    #[tanager(transparent)]
    struct Input(Files);

    impl std::fmt::Display for Input {
        #[inline]
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "mbed::include_str {}", self.0)
        }
    }

    mbed_macros::execute_fn(item.into(), |deps, input: Input| {
        deps.track_files(&*input.0);

        let mut writer = Writer::new()?;

        for path in input.0.iter() {
            std::io::copy(&mut File::open(path)?, &mut writer)?;
        }

        std::io::Result::Ok(Artifact {
            mime: input
                .0
                .first()
                .map(|x| {
                    mime_guess::from_path(x)
                        .first_or_text_plain()
                        .to_string()
                        .into()
                })
                .unwrap_or(Borrowed("text/plain")),

            ..writer.finish()?.into_text_artifact()
        })
    })
    .into()
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Parse)]
#[tanager(transparent)]
struct Files(Box<[LocalFile]>);

impl std::fmt::Display for Files {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cwd = std::env::current_dir().ok();

        let paths = self
            .0
            .iter()
            .map(|x| {
                cwd.as_ref()
                    .and_then(|cwd| x.strip_prefix(cwd).ok())
                    .unwrap_or(x.as_path())
            })
            .map(|x| x.to_string_lossy());

        write!(f, "[ ")?;

        let mut b: usize = 80;

        for (index, path) in paths.enumerate() {
            if index != 0 {
                write!(f, ", ")?;

                b = b.saturating_sub(path.len());

                if b != 0 {
                    write!(f, "{path}")?;
                } else {
                    write!(f, "...")?;
                }
            } else {
                b = b.saturating_sub(path.len());

                write!(f, "{path}")?;
            }
        }

        write!(f, " ]")?;

        Ok(())
    }
}

impl Deref for Files {
    type Target = [LocalFile];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
