use std::{
    collections::HashMap,
    fmt::Display,
    fs::File,
    hash::Hash,
    io::{BufReader, BufWriter, Seek},
    marker::PhantomData,
    path::Path,
    sync::Mutex,
    time::SystemTime,
};

use proc_macro_crate::FoundCrate;
use proc_macro2::Span;
use quote::{ToTokens, quote};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use sha2::{Digest, Sha256};
use syn::{Ident, spanned::Spanned};
use tanager::Parse;
use walkdir::WalkDir;

use crate::Artifact;

pub trait Embedder {
    type Value: ToTokens + Serialize + DeserializeOwned;

    type Input: Parse + Hash + Display;

    type Error: Into<Box<dyn std::error::Error + Send + Sync + 'static>>;

    fn execute(
        self,
        dependencies: &mut Dependencies,
        input: Self::Input,
    ) -> Result<Artifact<Self::Value>, Self::Error>;
}

pub struct Executor(Mutex<Cache>);

impl std::fmt::Debug for Executor {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Executor").finish()
    }
}

impl Default for Executor {
    #[inline]
    fn default() -> Self {
        Self(Mutex::new(Cache::new()))
    }
}

impl Executor {
    pub fn execute<E>(
        &self,
        tokens: proc_macro2::TokenStream,
        embedder: E,
    ) -> syn::Result<proc_macro2::TokenStream>
    where
        E: Embedder,
    {
        let span = tokens.span();

        if !tokens.is_empty() && span.local_file().is_none() {
            return Ok(quote! { ::core::unimplemented!() });
        }

        let input: E::Input = tanager::parse_without_container(tokens)?;

        let id = {
            let mut hasher = Sha256Hasher::new();

            input.hash(&mut hasher);

            typeid::of::<E>().hash(&mut hasher);

            Id(hasher.finish())
        };

        let cached = {
            let mut cache = self.0.lock().unwrap();

            cache.sync().unwrap();

            cache
                .entries
                .get(&id)
                .filter(|x| !x.dependencies.is_dirty())
                .and_then(|x| {
                    serde_json::from_value::<Artifact<E::Value>>(x.value.clone())
                        .ok()
                        .map(|v| (v, x.dependencies.to_token_stream()))
                })
        };

        let crate_ident =
            match proc_macro_crate::crate_name("mbed").map_err(|err| syn::Error::new(span, err))? {
                FoundCrate::Itself => quote! { crate },

                FoundCrate::Name(x) => Ident::new(&x, Span::call_site()).into_token_stream(),
            };

        match cached {
            Some((artifact, dependencies)) => Ok(quote! {{
                use #crate_ident as mbed;

                #dependencies
                #artifact
            }}),

            None => {
                crate::cargo::emit(format_args!("\x1b[1;36m    Bundling\x1b[0m {input}"));

                let mut dependencies = Dependencies::new();

                let artifact = embedder
                    .execute(&mut dependencies, input)
                    .map_err(|err| syn::Error::new(span, err.into()))?;

                let tokens = quote! {{
                    use #crate_ident as mbed;

                    #dependencies
                    #artifact
                }};

                if let Ok(value) = serde_json::to_value(&artifact) {
                    let mut cache = self.0.lock().unwrap();

                    cache.entries.insert(
                        id,
                        Entry {
                            created: SystemTime::now(),
                            dependencies,
                            value,
                        },
                    );

                    cache.sync().unwrap();
                }

                Ok(tokens)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
struct Id(#[serde(with = "const_hex")] [u8; 32]);

struct Cache {
    entries: HashMap<Id, Entry>,
    modified: SystemTime,
}

impl Cache {
    #[inline]
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            modified: SystemTime::UNIX_EPOCH,
        }
    }

    fn sync(&mut self) -> std::io::Result<()> {
        let path = crate::cargo::metadata().target_directory.join("mbed.lock");

        let mut file = match File::create_new(&path) {
            Ok(file) => file,

            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                let file = File::options().read(true).write(true).open(&path)?;

                file.lock()?;
                file
            }

            Err(err) => return Err(err),
        };

        let metadata = file.metadata()?;

        if let Ok(modified) = metadata.modified()
            && modified >= self.modified
        {
            let entries: HashMap<Id, Entry> = if metadata.len() > 0 {
                serde_json::from_reader(BufReader::new(&mut file))?
            } else {
                HashMap::new()
            };

            for (k, v) in entries {
                match self.entries.entry(k) {
                    std::collections::hash_map::Entry::Occupied(mut entry) => {
                        if v.created > entry.get().created {
                            entry.insert(v);
                        }
                    }

                    std::collections::hash_map::Entry::Vacant(entry) => {
                        entry.insert(v);
                    }
                }
            }

            file.rewind()?;
            file.set_len(0)?;

            serde_json::to_writer(BufWriter::new(&mut file), &self.entries)?;

            self.modified = modified;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Entry {
    created: SystemTime,
    dependencies: Dependencies,
    value: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dependencies {
    tracked_files: HashMap<Box<Path>, SystemTime>,
}

impl Dependencies {
    #[inline]
    fn new() -> Self {
        Self {
            tracked_files: HashMap::new(),
        }
    }

    #[inline]
    pub fn track_files<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item: AsRef<Path>>,
    {
        for path in iter {
            self.track_file(path);
        }
    }

    pub fn track_file<P>(&mut self, path: P)
    where
        P: AsRef<Path>,
    {
        if let Ok(path) = path.as_ref().canonicalize() {
            let timestamp = WalkDir::new(&path)
                .follow_links(true)
                .into_iter()
                .filter_map(|x| x.ok())
                .filter_map(|x| x.metadata().ok())
                .filter_map(|x| x.modified().ok())
                .max()
                .unwrap_or_else(SystemTime::now);

            self.tracked_files.insert(path.into_boxed_path(), timestamp);
        }
    }

    fn is_dirty(&self) -> bool {
        self.tracked_files.iter().any(|(k, v)| {
            WalkDir::new(k)
                .follow_links(true)
                .into_iter()
                .filter_map(|x| x.ok())
                .filter_map(|x| x.metadata().ok())
                .filter_map(|x| x.modified().ok())
                .max()
                .is_none_or(|x| &x > v)
        })
    }
}

impl ToTokens for Dependencies {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let paths = self
            .tracked_files
            .keys()
            .flat_map(|x| WalkDir::new(x).follow_links(true))
            .filter_map(|x| x.ok())
            .filter(|x| !x.file_type().is_dir())
            .filter_map(|x| x.into_path().to_str().map(|x| x.to_owned()));

        quote! {{
            #(
                #[allow(unused)]
                const _: &'static [u8] = ::core::include_bytes!(#paths);
            )*
        }}
        .to_tokens(tokens);
    }
}

struct Sha256Hasher(Sha256);

impl Sha256Hasher {
    #[inline]
    fn new() -> Self {
        Self(Sha256::new())
    }

    #[inline]
    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

impl std::hash::Hasher for Sha256Hasher {
    #[inline]
    fn finish(&self) -> u64 {
        u64::from_ne_bytes(*self.0.clone().finalize().first_chunk().unwrap())
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FnEmbedder<F, I, V, E>(F, PhantomData<(I, V, E)>);

impl<F, I, V, E> FnEmbedder<F, I, V, E> {
    #[inline]
    pub const fn new(f: F) -> Self {
        Self(f, PhantomData)
    }
}

impl<F, I, V, E> Embedder for FnEmbedder<F, I, V, E>
where
    F: FnOnce(&mut Dependencies, I) -> Result<Artifact<V>, E>,
    I: Parse + Hash + Display,
    V: ToTokens + Serialize + DeserializeOwned,
    E: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
{
    type Value = V;

    type Input = I;

    type Error = E;

    #[inline]
    fn execute(
        self,
        dependencies: &mut Dependencies,
        input: Self::Input,
    ) -> Result<Artifact<Self::Value>, Self::Error> {
        (self.0)(dependencies, input)
    }
}
