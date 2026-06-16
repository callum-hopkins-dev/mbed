use std::{borrow::Cow, io::Write, path::Path};

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha224};
use tempfile::NamedTempFile;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Artifact<T> {
    pub mime: Cow<'static, str>,
    pub id: Id,
    pub value: T,
}

impl<T> ToTokens for Artifact<T>
where
    T: ToTokens,
{
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self { mime, id, value } = self;

        quote! {
            mbed::__macro::artifact(#mime, #id, #value)
        }
        .to_tokens(tokens);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Id(#[serde(with = "const_hex")] [u8; 28]);

impl Id {
    #[inline]
    pub const fn from_bytes(x: [u8; 28]) -> Self {
        Self(x)
    }

    #[inline]
    pub const fn as_bytes(&self) -> &[u8; 28] {
        &self.0
    }

    #[inline]
    pub const fn to_bytes(self) -> [u8; 28] {
        self.0
    }
}

impl ToTokens for Id {
    #[inline]
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let bytes = self.as_bytes();
        quote! { mbed::Id::from_bytes([ #(#bytes),* ]) }.to_tokens(tokens);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Bytes(Box<Path>);

impl ToTokens for Bytes {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let path = self.0.to_str().unwrap();

        quote! {
            ::core::include_bytes!(#path)
        }
        .to_tokens(tokens);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Text(Box<Path>);

impl ToTokens for Text {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let path = self.0.to_str().unwrap();

        quote! {
            ::core::include_str!(#path)
        }
        .to_tokens(tokens);
    }
}

impl Artifact<Bytes> {
    #[inline]
    pub fn into_text_artifact(self) -> Artifact<Text> {
        Artifact {
            mime: self.mime,
            id: self.id,
            value: Text(self.value.0),
        }
    }
}

impl Artifact<Text> {
    #[inline]
    pub fn into_bytes_artifact(self) -> Artifact<Bytes> {
        Artifact {
            mime: self.mime,
            id: self.id,
            value: Bytes(self.value.0),
        }
    }
}

#[derive(Debug)]
pub struct Writer {
    file: NamedTempFile,
    sha224: Sha224,
}

impl Writer {
    #[inline]
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {
            file: NamedTempFile::new_in(crate::cargo::metadata().target_directory.as_path())?,
            sha224: Sha224::new(),
        })
    }

    pub fn finish(mut self) -> std::io::Result<Artifact<Bytes>> {
        self.flush()?;

        let id = Id::from_bytes(self.sha224.finalize().into());

        let mut path = crate::cargo::metadata().target_directory.join("mbed");

        std::fs::create_dir_all(&path)?;

        path.push(
            const_hex::Buffer::<_, false>::new()
                .const_format(id.as_bytes())
                .as_str(),
        );

        self.file.persist(&path)?;

        Ok(Artifact {
            mime: Cow::Borrowed("application/octet-stream"),
            id,
            value: Bytes(path.into_std_path_buf().into_boxed_path()),
        })
    }
}

impl std::io::Write for Writer {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let len = self.file.write(buf)?;
        self.sha224.update(&buf[..len]);

        Ok(len)
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}
