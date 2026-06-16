use std::{fmt::Display, hash::Hash, sync::OnceLock};

use quote::ToTokens;
use serde::{Serialize, de::DeserializeOwned};
use tanager::Parse;

use crate::embedder::FnEmbedder;

pub use crate::{
    artifact::{Artifact, Bytes, Id, Text, Writer},
    embedder::{Dependencies, Embedder, Executor},
    local_file::LocalFile,
};

pub mod cargo;
pub mod local_file;

pub mod artifact;
pub mod embedder;

#[inline]
pub fn execute<E>(tokens: proc_macro2::TokenStream, embedder: E) -> proc_macro2::TokenStream
where
    E: Embedder,
{
    static EXECUTOR: OnceLock<Executor> = OnceLock::new();

    match EXECUTOR
        .get_or_init(Executor::default)
        .execute(tokens, embedder)
    {
        Ok(x) => x,
        Err(x) => x.into_compile_error(),
    }
}

#[inline]
pub fn execute_fn<F, I, V, E>(tokens: proc_macro2::TokenStream, f: F) -> proc_macro2::TokenStream
where
    F: FnOnce(&mut Dependencies, I) -> Result<Artifact<V>, E>,
    I: Parse + Hash + Display,
    V: ToTokens + Serialize + DeserializeOwned,
    E: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
{
    crate::execute(tokens, FnEmbedder::new(f))
}
