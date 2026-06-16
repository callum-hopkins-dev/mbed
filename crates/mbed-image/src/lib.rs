use std::{
    borrow::Cow::Borrowed,
    io::{Cursor, Write},
};

#[cfg(feature = "avif")]
use image::codecs::avif::AvifEncoder;
use image::{
    DynamicImage, ImageDecoder, ImageEncoder, ImageError, ImageFormat, ImageReader, RgbaImage,
    codecs::{
        bmp::BmpEncoder, farbfeld::FarbfeldEncoder, gif::GifEncoder, hdr::HdrEncoder,
        ico::IcoEncoder, jpeg::JpegEncoder, openexr::OpenExrEncoder, png::PngEncoder,
        pnm::PnmEncoder, qoi::QoiEncoder, tga::TgaEncoder, tiff::TiffEncoder, webp::WebPEncoder,
    },
    error::ImageFormatHint,
    imageops::FilterType,
};
use mbed_macros::{Artifact, Bytes, LocalFile, Writer};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use serde::{Deserialize, Serialize};
use tanager::Parse;

#[proc_macro]
pub fn include(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    mbed_macros::execute_fn(item.into(), |deps, input: Input| {
        deps.track_file(&input.path);

        let reader = ImageReader::open(&input.path)?;

        let format = input
            .format
            .or_else(|| reader.format().and_then(|x| x.try_into().ok()))
            .ok_or_else(|| ImageError::Unsupported(ImageFormatHint::Unknown.into()))?;

        let mut decoder = reader.into_decoder()?;
        let orientation = decoder.orientation()?;

        let mut image = DynamicImage::from_decoder(decoder)?;

        image.apply_orientation(orientation);

        let image = match input.resize {
            Some(Resize::Exact {
                keep_aspect_ratio: true,
                width,
                height,
            }) => image.resize(width, height, FilterType::Lanczos3),

            Some(Resize::Exact {
                keep_aspect_ratio: false,
                width,
                height,
            }) => image.resize_exact(width, height, FilterType::Lanczos3),

            Some(Resize::Scale(x)) => image.resize(
                (image.width() as f64 * x) as u32,
                (image.height() as f64 * x) as u32,
                FilterType::Lanczos3,
            ),

            None => image,
        };

        let (width, height) = (image.width(), image.height());

        let bytes = encode(&image.into_rgba8(), format, input.compression)?;

        let mut writer = Writer::new()?;
        writer.write_all(&bytes)?;
        let artifact = writer.finish()?;

        Result::Ok(Artifact {
            mime: Borrowed(format.mime()),
            id: artifact.id,

            value: Image {
                format,
                width,
                height,
                bytes: artifact.value,
            },
        })
    })
    .into()
}

fn encode(image: &RgbaImage, format: Format, compression: Compression) -> Result<Vec<u8>> {
    macro_rules! encode {
        ($image:expr, $encoder:expr) => {{
            $encoder.write_image(
                $image.as_raw(),
                $image.width(),
                $image.height(),
                image::ExtendedColorType::Rgba8,
            )?;
        }};
    }

    let mut bytes = Vec::new();

    match format {
        Format::Png => encode!(
            image,
            PngEncoder::new_with_quality(
                Cursor::new(&mut bytes),
                match compression {
                    Compression::Best => image::codecs::png::CompressionType::Best,
                    Compression::Fast => image::codecs::png::CompressionType::Fast,
                    Compression::Uncompressed => image::codecs::png::CompressionType::Uncompressed,
                    Compression::Balanced => image::codecs::png::CompressionType::Default,
                },
                image::codecs::png::FilterType::Adaptive,
            )
        ),

        Format::Jpeg => encode!(
            image,
            JpegEncoder::new_with_quality(
                Cursor::new(&mut bytes),
                match compression {
                    Compression::Best => 95,
                    Compression::Fast => 65,
                    Compression::Uncompressed => 100,
                    Compression::Balanced => 80,
                },
            )
        ),

        Format::Gif => encode!(
            image,
            GifEncoder::new_with_speed(
                Cursor::new(&mut bytes),
                match compression {
                    Compression::Best => 1,
                    Compression::Fast => 25,
                    Compression::Uncompressed => 30,
                    Compression::Balanced => 10,
                }
            )
        ),

        Format::WebP => encode!(image, WebPEncoder::new_lossless(Cursor::new(&mut bytes))),
        Format::Pnm => encode!(image, PnmEncoder::new(Cursor::new(&mut bytes))),
        Format::Tiff => encode!(image, TiffEncoder::new(Cursor::new(&mut bytes))),
        Format::Tga => encode!(image, TgaEncoder::new(Cursor::new(&mut bytes))),
        Format::Bmp => encode!(image, BmpEncoder::new(&mut Cursor::new(&mut bytes))),
        Format::Ico => encode!(image, IcoEncoder::new(Cursor::new(&mut bytes))),
        Format::Hdr => encode!(image, HdrEncoder::new(Cursor::new(&mut bytes))),
        Format::OpenExr => encode!(image, OpenExrEncoder::new(Cursor::new(&mut bytes))),
        Format::Farbfeld => encode!(image, FarbfeldEncoder::new(Cursor::new(&mut bytes))),

        #[cfg(feature = "avif")]
        Format::Avif => {
            let (speed, quality) = match compression {
                Compression::Best => (1, 95),
                Compression::Fast => (10, 65),
                Compression::Uncompressed => (1, 100),
                Compression::Balanced => (6, 80),
            };

            encode!(
                image,
                AvifEncoder::new_with_speed_quality(Cursor::new(&mut bytes), speed, quality)
            )
        }

        Format::Qoi => encode!(image, QoiEncoder::new(Cursor::new(&mut bytes))),
    };

    Ok(bytes)
}

#[derive(Debug, Hash, Parse)]
struct Input {
    #[tanager(default)]
    format: Option<Format>,

    path: LocalFile,

    #[tanager(default = Compression::Balanced)]
    compression: Compression,

    #[tanager(default)]
    resize: Option<Resize>,
}

impl std::fmt::Display for Input {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "mbed::image::include {{ path: {} }}",
            std::env::current_dir()
                .ok()
                .and_then(|cwd| self.path.strip_prefix(cwd).ok())
                .unwrap_or(self.path.as_path())
                .to_string_lossy()
        )
    }
}

#[derive(Debug, Parse)]
enum Resize {
    Exact {
        #[tanager(default = true)]
        keep_aspect_ratio: bool,

        width: u32,
        height: u32,
    },

    Scale(f64),
}

impl std::hash::Hash for Resize {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        ::core::mem::discriminant(self).hash(state);

        match self {
            Self::Exact {
                keep_aspect_ratio,
                width,
                height,
            } => {
                keep_aspect_ratio.hash(state);
                width.hash(state);
                height.hash(state);
            }

            Resize::Scale(x) => {
                x.to_bits().hash(state);
            }
        }
    }
}

type Result<T> = ::core::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error(transparent)]
    Image(#[from] image::ImageError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Serialize, Deserialize)]
struct Image {
    format: Format,
    width: u32,
    height: u32,
    bytes: Bytes,
}

impl ToTokens for Image {
    #[inline]
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self {
            format,
            width,
            height,
            bytes,
        } = self;

        quote! {{
            const IMAGE: mbed::image::__macro::Image = mbed::image::__macro::image(#format, #width, #height, #bytes);

            &IMAGE
        }}
        .to_tokens(tokens);
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Parse, Serialize, Deserialize,
)]
enum Format {
    Png,
    Jpeg,
    Gif,
    WebP,
    Pnm,
    Tiff,
    Tga,
    Bmp,
    Ico,
    Hdr,
    OpenExr,
    Farbfeld,
    #[cfg(feature = "avif")]
    Avif,
    Qoi,
}

impl Format {
    #[inline]
    const fn mime(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::WebP => "image/webp",
            Self::Pnm => "image/x-portable-anymap",
            Self::Tiff => "image/tiff",
            Self::Tga => "image/x-tga",
            Self::Bmp => "image/bmp",
            Self::Ico => "image/vnd.microsoft.icon",
            Self::Hdr => "image/vnd.radiance",
            Self::OpenExr => "image/x-exr",
            Self::Farbfeld => "image/farbfeld",
            #[cfg(feature = "avif")]
            Self::Avif => "image/avif",
            Self::Qoi => "image/qoi",
        }
    }
}

impl ToTokens for Format {
    #[inline]
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::Png => quote! { mbed::image::Format::Png }.to_tokens(tokens),
            Self::Jpeg => quote! { mbed::image::Format::Jpeg }.to_tokens(tokens),
            Self::Gif => quote! { mbed::image::Format::Gif }.to_tokens(tokens),
            Self::WebP => quote! { mbed::image::Format::WebP }.to_tokens(tokens),
            Self::Pnm => quote! { mbed::image::Format::Pnm }.to_tokens(tokens),
            Self::Tiff => quote! { mbed::image::Format::Tiff }.to_tokens(tokens),
            Self::Tga => quote! { mbed::image::Format::Tga }.to_tokens(tokens),
            Self::Bmp => quote! { mbed::image::Format::Bmp }.to_tokens(tokens),
            Self::Ico => quote! { mbed::image::Format::Ico }.to_tokens(tokens),
            Self::Hdr => quote! { mbed::image::Format::Hdr }.to_tokens(tokens),
            Self::OpenExr => quote! { mbed::image::Format::OpenExr }.to_tokens(tokens),
            Self::Farbfeld => quote! { mbed::image::Format::Farbfeld }.to_tokens(tokens),
            #[cfg(feature = "avif")]
            Self::Avif => quote! { mbed::image::Format::Avif }.to_tokens(tokens),
            Self::Qoi => quote! { mbed::image::Format::Qoi }.to_tokens(tokens),
        }
    }
}

impl TryFrom<ImageFormat> for Format {
    type Error = ImageError;

    fn try_from(value: ImageFormat) -> std::result::Result<Self, Self::Error> {
        match value {
            ImageFormat::Png => Ok(Format::Png),
            ImageFormat::Jpeg => Ok(Format::Jpeg),
            ImageFormat::Gif => Ok(Format::Gif),
            ImageFormat::WebP => Ok(Format::WebP),
            ImageFormat::Pnm => Ok(Format::Pnm),
            ImageFormat::Tiff => Ok(Format::Tiff),
            ImageFormat::Tga => Ok(Format::Tga),
            ImageFormat::Bmp => Ok(Format::Bmp),
            ImageFormat::Ico => Ok(Format::Ico),
            ImageFormat::Hdr => Ok(Format::Hdr),
            ImageFormat::OpenExr => Ok(Format::OpenExr),
            ImageFormat::Farbfeld => Ok(Format::Farbfeld),
            #[cfg(feature = "avif")]
            ImageFormat::Avif => Ok(Format::Avif),
            ImageFormat::Qoi => Ok(Format::Qoi),

            _ => Err(ImageError::Unsupported(ImageFormatHint::Unknown.into())),
        }
    }
}

impl From<Format> for ImageFormat {
    fn from(value: Format) -> Self {
        match value {
            Format::Png => Self::Png,
            Format::Jpeg => Self::Jpeg,
            Format::Gif => Self::Gif,
            Format::WebP => Self::WebP,
            Format::Pnm => Self::Pnm,
            Format::Tiff => Self::Tiff,
            Format::Tga => Self::Tga,
            Format::Bmp => Self::Bmp,
            Format::Ico => Self::Ico,
            Format::Hdr => Self::Hdr,
            Format::OpenExr => Self::OpenExr,
            Format::Farbfeld => Self::Farbfeld,
            #[cfg(feature = "avif")]
            Format::Avif => Self::Avif,
            Format::Qoi => Self::Qoi,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Parse, Serialize, Deserialize,
)]
enum Compression {
    Best,
    Fast,
    Uncompressed,
    Balanced,
}
