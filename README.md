<div align="center">

# mbed

Embed and transform assets into your Rust crate.

[![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/callum-hopkins-dev/mbed/build.yaml?branch=main&event=push&style=for-the-badge)](https://github.com/callum-hopkins-dev/mbed/actions/workflows/build.yaml)
[![Crates.io Version](https://img.shields.io/crates/v/mbed?style=for-the-badge)](https://crates.io/crates/mbed)
[![docs.rs](https://img.shields.io/docsrs/mbed?style=for-the-badge)](https://docs.rs/mbed/latest/mbed)
[![Crates.io Total Downloads](https://img.shields.io/crates/d/mbed?style=for-the-badge)](https://crates.io/crates/mbed)
[![GitHub License](https://img.shields.io/github/license/callum-hopkins-dev/mbed?style=for-the-badge)](https://github.com/callum-hopkins-dev/mbed/blob/main/LICENSE)

</div>

## about

`mbed` provides macros for turning files and generated outputs into `Artifact`
values. Artifacts carry their bytes, MIME type, and `Id`, and can be grouped
into a `Manifest` for lookup by identifier.

### Basic usage

Requires the `macros` feature.

```rust
pub const LOGO: Artifact<[u8]> = mbed::include_bytes!["../assets/logo.svg"];
pub const INDEX: Artifact<str> = mbed::include_str!["../pages/index.html"];

let id = LOGO.id();
assert_eq!(LOGO.mime(), "image/svg+xml");
```

### Collections and manifests

`collect!` groups artifacts inside a module. `manifest!` can then reference
individual artifacts or whole collections with `::*`.

Requires the `macros` feature.

```rust
pub mod pages {
    pub const INDEX: Artifact<str> = mbed::include_str!["../pages/index.html"];
    pub const ABOUT: Artifact<str> = mbed::include_str!["../pages/about.html"];

    mbed::collect![INDEX, ABOUT];
}

pub mod images {
    pub const LOGO: Artifact<[u8]> = mbed::include_bytes!["../assets/logo.svg"];

    mbed::collect![LOGO];
}

pub const ASSETS: Manifest = mbed::manifest![pages::*, images::*];

let artifact = ASSETS.get(pages::INDEX.id()).unwrap();
assert_eq!(artifact.mime(), pages::INDEX.mime());
```

### Images

Requires the `image` feature.

```rust
pub const LOGO: Artifact<Image> = mbed::image::include! {
    resize: Scale(0.5),
    path: "../assets/logo.png",
    format: WebP,
};

assert_eq!(LOGO.format(), mbed::image::Format::WebP);
let bytes = LOGO.as_bytes();
```

### JavaScript bundles

Requires the `bundle` feature.

```rust
pub const APP: Artifact<Bundle> = mbed::js::bundle!["../frontend/app.js"];

let code = APP.code();
let sourcemap = APP.sourcemap();
```

### Tailwind CSS

Requires the `tailwindcss` feature.

```rust
pub const STYLES: Artifact<str> = mbed::css::tailwindcss!("../tailwind.css");
```

### Feature flags

- `macros` enables `include_bytes!`, `include_str!`, `collect!`, and
  `manifest!`.
- `image` enables `image:include!` and image metadata support.
- `bundle` enables `js:bundle!`.
- `tailwindcss` enables `css:tailwindcss!`.
- `serde` enables serialization support for public data types.
- `cargo-progress` shows bundling progress in Cargo build output. This feature
  is only supported on Linux.

## License

`mbed` is licensed under the MIT License. See `LICENSE` for details.

## Contributing

Contributions are welcome.

Please follow the existing code style and conventions used throughout the
project. If you're proposing a new feature or API, opening an issue first is
often the easiest way to discuss the design.
