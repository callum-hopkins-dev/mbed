use std::{borrow::Cow::Borrowed, io::Write, ops::Deref};

use mbed_macros::{Artifact, LocalFile, Text, Writer};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use rolldown::{
    Bundler, BundlerOptions, CodeSplittingMode, CommentsOptions, InputItem, OutputFormat,
    RawMinifyOptions, SourceMapType, TreeshakeOptions,
};
use rolldown_common::Output;
use rolldown_error::BatchedBuildDiagnostic;
use serde::{Deserialize, Serialize};
use tanager::Parse;

pub fn proc_macro(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    mbed_macros::execute_fn(item.into(), |deps, input: Input| -> Result<_, Error> {
        let mut bundler = Bundler::new(BundlerOptions {
            input: Some(
                input
                    .iter()
                    .filter_map(|x| x.to_str())
                    .map(|x| InputItem {
                        import: x.to_owned(),
                        ..Default::default()
                    })
                    .collect(),
            ),

            treeshake: TreeshakeOptions::Boolean(true),
            minify: Some(RawMinifyOptions::Bool(true)),
            sourcemap: Some(SourceMapType::Hidden),
            format: Some(OutputFormat::Iife),
            code_splitting: Some(CodeSplittingMode::Bool(false)),

            comments: Some(CommentsOptions {
                legal: true,
                annotation: false,
                jsdoc: false,
            }),

            ..Default::default()
        })?;

        let bundle = tokio::runtime::LocalRuntime::new()?.block_on(bundler.generate())?;

        deps.track_files(bundler.watch_files().iter().map(|x| x.to_string()));

        let code = {
            let mut writer = Writer::new()?;

            let bytes = bundle
                .assets
                .iter()
                .find_map(|x| match x {
                    Output::Chunk(x) if x.is_entry => Some(x.code.clone()),

                    _ => None,
                })
                .unwrap()
                .into_bytes();

            writer.write_all(&bytes)?;
            writer.finish()?.into_text_artifact()
        };

        let sourcemap = {
            let mut writer = Writer::new()?;

            let bytes = bundle
                .assets
                .iter()
                .find_map(|x| match x {
                    Output::Asset(x) if x.filename.ends_with(".js.map") => Some(x.source.clone()),

                    _ => None,
                })
                .unwrap();

            writer.write_all(bytes.as_bytes())?;
            writer.finish()?.into_text_artifact()
        };

        Ok(Artifact {
            mime: Borrowed("application/javascript"),
            id: code.id,
            value: Bundle {
                code: code.value,
                sourcemap: sourcemap.value,
            },
        })
    })
    .into()
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Parse)]
#[tanager(transparent)]
struct Input(Box<[LocalFile]>);

impl std::fmt::Display for Input {
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

        write!(f, "mbed::js::bundle [ ")?;

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

impl Deref for Input {
    type Target = [LocalFile];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error(transparent)]
    Rolldown(RolldownError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<BatchedBuildDiagnostic> for Error {
    #[inline]
    fn from(value: BatchedBuildDiagnostic) -> Self {
        Self::Rolldown(RolldownError(value))
    }
}

#[derive(Debug)]
struct RolldownError(BatchedBuildDiagnostic);

impl std::fmt::Display for RolldownError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for x in self.0.iter() {
            write!(f, "{}", x.to_diagnostic().convert_to_string(false))?;
        }

        Ok(())
    }
}

impl std::error::Error for RolldownError {}

#[derive(Debug, Serialize, Deserialize)]
struct Bundle {
    code: Text,
    sourcemap: Text,
}

impl ToTokens for Bundle {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self { code, sourcemap } = self;

        quote! {{
            const BUNDLE: mbed::js::__macro::Bundle = mbed::js::__macro::bundle(#code, #sourcemap);

            &BUNDLE
        }}
        .to_tokens(tokens);
    }
}
