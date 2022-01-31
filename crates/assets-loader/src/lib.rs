#![warn(
    // missing_docs,
    // rustdoc::missing_doc_code_examples,
    future_incompatible,
    rust_2018_idioms,
    unused,
    trivial_casts,
    trivial_numeric_casts,
    unused_lifetimes,
    unused_qualifications,
    unused_crate_dependencies,
    clippy::cargo,
    clippy::multiple_crate_versions,
    clippy::empty_line_after_outer_attr,
    clippy::fallible_impl_from,
    clippy::redundant_pub_crate,
    clippy::use_self,
    clippy::suspicious_operation_groupings,
    clippy::useless_let_if_seq,
    // clippy::missing_errors_doc,
    // clippy::missing_panics_doc,
    clippy::wildcard_imports
)]
#![doc(html_logo_url = "https://raw.githubusercontent.com/HellButcher/pulz/master/docs/logo.png")]
#![doc(html_no_source)]
#![doc = include_str!("../README.md")]

use std::{future::Future, io::Cursor};

use path::{AssetPath, AssetPathBuf};
use platform::AssetOpen;

pub mod path;
pub mod platform;

pub trait LoadAsset<A> {
    type Future: Future<Output = Result<A, Self::Error>>;
    type Error: From<std::io::Error>;
    fn load(&self, load: Load<'_>) -> Self::Future;
}

impl<A, E, F, T> LoadAsset<A> for T
where
    T: Fn(Load<'_>) -> F,
    F: Future<Output = Result<A, E>>,
    E: From<std::io::Error>,
{
    type Future = F;
    type Error = E;
    fn load(&self, load: Load<'_>) -> Self::Future {
        self(load)
    }
}

pub struct Load<'a> {
    buffer: Cursor<Vec<u8>>,
    path: AssetPathBuf,
    server: &'a AssetServer,
}

impl Load<'_> {
    #[inline]
    pub fn path(&self) -> &AssetPath {
        &self.path
    }

    #[inline]
    pub fn into_vec(self) -> Vec<u8> {
        self.buffer.into_inner()
    }

    #[inline]
    pub fn cursor_mut(&mut self) -> &'_ mut Cursor<impl AsRef<[u8]>> {
        &mut self.buffer
    }

    #[inline]
    pub fn as_slice(&self) -> &'_ [u8] {
        self.buffer.get_ref()
    }

    #[inline]
    pub async fn load(&self, path: impl AsRef<AssetPath>) -> std::io::Result<Load<'_>> {
        self._load(path.as_ref()).await
    }
    async fn _load(&self, path: &AssetPath) -> std::io::Result<Load<'_>> {
        let full_path = if self.path.is_directory() {
            self.path.join(path)
        } else if let Some(parent) = self.path.parent() {
            parent.join(path)
        } else {
            let mut p = AssetPathBuf::from("/");
            p.push(path);
            p
        };
        self.server._load(full_path).await
    }
}

impl AsRef<[u8]> for Load<'_> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

pub struct AssetServer {
    io: Box<dyn AssetOpen>,
}

impl AssetServer {
    pub fn new() -> Self {
        Self::with(platform::default_platform_io())
    }
    #[inline]
    pub fn with(io: impl AssetOpen + 'static) -> Self {
        Self { io: Box::new(io) }
    }
}

impl AssetServer {
    pub async fn load_with<A, L: LoadAsset<A>>(
        &self,
        path: impl AsRef<AssetPath>,
        loader: L,
    ) -> Result<A, L::Error> {
        let load = self.load(path).await?;
        loader.load(load).await
    }

    pub async fn load(&self, path: impl AsRef<AssetPath>) -> std::io::Result<Load<'_>> {
        let mut abs_base = AssetPathBuf::from("/");
        abs_base.push(path);
        self._load(abs_base).await
    }

    async fn _load(&self, path: AssetPathBuf) -> std::io::Result<Load<'_>> {
        let buffer = self.io.load(&path).await?;
        Ok(Load {
            buffer: Cursor::new(buffer),
            path,
            server: self,
        })
    }
}
