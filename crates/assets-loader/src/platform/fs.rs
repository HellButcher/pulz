use std::{
    fs::{File, Metadata},
    io::Read,
    path::PathBuf,
};

use super::{AssetOpen, BoxedFuture};
use crate::path::{AssetPath, Component};

pub struct FileSystemAssetLoaderIo {
    dirs: Vec<PathBuf>,
}

fn join_path(base_path: &mut PathBuf, asset_path: &AssetPath) {
    // TODO: check for invalid characters in path components (except fragment) ([a-zA-Z0-9._-])
    base_path.reserve_exact(asset_path.as_str().len());
    let mut num_comps = 0; // count components to not escape from `base_path`
    for comp in asset_path.components() {
        match comp {
            Component::Directory(dir) | Component::File(dir) => {
                num_comps += 1;
                base_path.push(dir);
            }
            Component::ParentDir => {
                if num_comps > 0 {
                    num_comps -= 1;
                    base_path.pop();
                }
            }
            Component::RootDir | Component::Fragment(_) => {}
        }
    }
}

impl FileSystemAssetLoaderIo {
    pub const fn new() -> Self {
        Self { dirs: Vec::new() }
    }

    #[inline]
    pub fn with_directory(mut self, path: impl Into<PathBuf>) -> Self {
        self._push_directory(path.into());
        self
    }

    #[inline]
    pub fn push_directory(&mut self, path: impl Into<PathBuf>) -> &mut Self {
        self._push_directory(path.into());
        self
    }

    fn _push_directory(&mut self, path: PathBuf) {
        self.dirs.push(path);
    }

    async fn resolve_path(&self, asset_path: &AssetPath) -> Option<(PathBuf, Metadata)> {
        for base in &self.dirs {
            let mut path = base.to_owned();
            join_path(&mut path, asset_path);
            if let Some(r) = blocking::unblock(move || {
                let metadata = path.metadata().ok()?;
                Some((path, metadata))
            })
            .await
            {
                return Some(r);
            }
        }
        None
    }

    async fn open(&self, asset_path: &AssetPath) -> std::io::Result<Vec<u8>> {
        let (full_path, metadata) = self
            .resolve_path(asset_path)
            .await
            .ok_or(std::io::ErrorKind::NotFound)?;
        let len = metadata.len();
        if len >= usize::MAX as u64 {
            // TODO: smaller max file size?
            // TODO: use this when stabelized
            // return Err(std::io::ErrorKind::FileTooLarge.into());
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "file to large",
            ));
        }
        blocking::unblock(move || {
            let mut file = File::open(full_path)?;
            let mut buf = Vec::with_capacity(len as usize);
            file.read_to_end(&mut buf)?;
            Ok(buf)
        })
        .await
    }
}

impl AssetOpen for FileSystemAssetLoaderIo {
    fn load(&self, asset_path: &AssetPath) -> BoxedFuture<'_, std::io::Result<Vec<u8>>> {
        let asset_path = asset_path.to_owned();
        Box::pin(async move { self.open(&asset_path).await })
    }
}
