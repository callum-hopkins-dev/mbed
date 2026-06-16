use std::{hash::Hash, ops::Deref, path::Path};

use serde::{Deserialize, Serialize};
use syn::LitStr;
use tanager::Parse;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LocalFile(Box<Path>);

impl LocalFile {
    #[inline]
    pub const fn as_path(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for LocalFile {
    #[inline]
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl Deref for LocalFile {
    type Target = Path;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_path()
    }
}

impl syn::parse::Parse for LocalFile {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lit: LitStr = input.parse()?;

        let local_file = lit
            .span()
            .local_file()
            .ok_or_else(|| syn::Error::new(lit.span(), "could not resolve local file"))?;

        let mut path = std::env::current_dir().map_err(|x| syn::Error::new(lit.span(), x))?;

        path.push(local_file);
        path.pop();

        path.push(lit.value());

        Ok(Self(
            path.canonicalize()
                .map_err(|x| syn::Error::new(lit.span(), x))?
                .into_boxed_path(),
        ))
    }
}

impl Parse for LocalFile {
    #[inline]
    fn parse(input: tanager::ParseStream<'_>) -> tanager::Result<Self> {
        syn::parse::Parse::parse(input)
    }
}
