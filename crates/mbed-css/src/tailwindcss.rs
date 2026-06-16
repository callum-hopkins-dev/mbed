use std::{borrow::Cow::Borrowed, io::Write, process::Command, string::FromUtf8Error};

use mbed_macros::{Artifact, LocalFile, Writer};
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{tag, take_until},
    character::complete::{char, multispace0},
    multi::many0,
    sequence::{delimited, preceded},
};
use tanager::Parse;
use walkdir::WalkDir;

pub fn proc_macro(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    mbed_macros::execute_fn(item.into(), |deps, input: Input| {
        deps.track_file(&input.0);

        let stylesheet = std::fs::read_to_string(&input.0)?;
        let sources = sources(&stylesheet);

        if sources.is_empty() {
            deps.track_files(
                WalkDir::new(std::env::current_dir()?)
                    .max_depth(1)
                    .into_iter()
                    .filter_map(|x| x.ok())
                    .filter(|x| {
                        x.depth() == 1
                            && x.file_name() != "Cargo.toml"
                            && x.file_name() != "Cargo.lock"
                            && x.file_name() != "target"
                    })
                    .map(|x| x.into_path()),
            );
        }

        deps.track_files(&sources);

        let output = Command::new(which::which("tailwindcss")?)
            .arg("--input")
            .arg(&*input.0)
            .arg("--minify")
            .output()?;

        if output.status.success() {
            let mut writer = Writer::new()?;
            writer.write_all(&output.stdout)?;

            Ok(Artifact {
                mime: Borrowed("text/css"),
                ..writer.finish()?
            }
            .into_text_artifact())
        } else {
            Err(Error::Tailwind(
                String::from_utf8(output.stderr)?.into_boxed_str(),
            ))
        }
    })
    .into()
}

#[derive(Debug, Hash, Parse)]
#[tanager(transparent)]
struct Input(LocalFile);

impl std::fmt::Display for Input {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "mbed::css::tailwindcss({})",
            std::env::current_dir()
                .ok()
                .and_then(|cwd| self.0.strip_prefix(cwd).ok())
                .unwrap_or(self.0.as_path())
                .to_string_lossy()
        )
    }
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Which(#[from] which::Error),

    #[error(transparent)]
    FromUtf8(#[from] FromUtf8Error),

    #[error("{0}")]
    Tailwind(Box<str>),
}

fn sources(mut s: &str) -> Vec<&str> {
    fn quoted(input: &str) -> IResult<&str, &str> {
        alt((
            delimited(char('"'), take_until("\""), char('"')),
            delimited(char('\''), take_until("'"), char('\'')),
        ))
        .parse(input)
    }

    // Parse source("...") or source('...')
    fn source_fn(input: &str) -> IResult<&str, &str> {
        preceded(
            (multispace0, tag("source"), multispace0),
            delimited(
                char('('),
                delimited(multispace0, quoted, multispace0),
                char(')'),
            ),
        )
        .parse(input)
    }

    // Parse @source "..." OR @source("...")
    fn source_at(input: &str) -> IResult<&str, &str> {
        preceded(
            (multispace0, tag("@source"), multispace0),
            alt((
                quoted,
                delimited(
                    char('('),
                    delimited(multispace0, quoted, multispace0),
                    char(')'),
                ),
            )),
        )
        .parse(input)
    }

    // Parse @import "tailwindcss" ... source(...)
    fn import(input: &str) -> IResult<&str, Vec<&str>> {
        let (input, _) = (
            multispace0,
            tag("@import"),
            multispace0,
            alt((tag("\"tailwindcss\""), tag("\'tailwindcss\'"))),
        )
            .parse(input)?;

        // Scan for any number of source(...) calls
        many0(source_fn).parse(input)
    }

    let mut x = Vec::new();

    while !s.is_empty() {
        if let Ok((next, src)) = source_at(s) {
            x.push(src);
            s = next;
        } else if let Ok((next, mut srcs)) = import(s) {
            x.append(&mut srcs);
            s = next;
        } else {
            s = &s[1..];
        }
    }

    x
}
