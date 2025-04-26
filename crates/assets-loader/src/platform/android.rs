use std::{borrow::Cow, ffi::CString, io::Read};

use ndk::asset::{Asset as AndroidAsset, AssetManager as AndroidAssetManager};

use crate::{
    path::{AssetPath, AssetPathBuf},
    platform::{AssetOpen, BoxedFuture},
};

pub struct AndroidAssetLoaderIo {
    manager: AndroidAssetManager,
    base: Cow<'static, str>,
}

fn clone_mgr(mgr: &AndroidAssetManager) -> AndroidAssetManager {
    // SAFETY: pointer is a valid asset manager!
    unsafe { AndroidAssetManager::from_ptr(mgr.ptr()) }
}

impl AndroidAssetLoaderIo {
    #[inline]
    pub const fn with_asset_manager(manager: AndroidAssetManager) -> Self {
        Self {
            manager,
            base: Cow::Borrowed(""),
        }
    }

    #[inline]
    pub fn with_base(mut self, base: impl Into<Cow<'static, str>>) -> Self {
        self.set_base(base);
        self
    }

    #[inline]
    pub fn set_base(&mut self, base: impl Into<Cow<'static, str>>) -> &mut Self {
        self.base = base.into();
        self
    }

    fn resolve_path(&self, asset_path: &AssetPath) -> String {
        let resolved_path = self.resolve_path(asset_path);

        // make absolute & normalize
        let mut norm_asset_path = AssetPathBuf::with_capacity(asset_path.len() + 1);
        norm_asset_path.push("/");
        norm_asset_path.push(asset_path);

        let base = self.base.trim_end_matches('/');
        let mut result = String::with_capacity(base.len() + asset_path.len());
        result.push_str(base);
        result.push_str(&norm_asset_path.as_str());
        result
    }

    async fn open(&self, asset_path: &AssetPath) -> std::io::Result<Vec<u8>> {
        let resolved_path = self.resolve_path(asset_path);
        let resolved_path = CString::new(resolved_path.into_bytes())?;
        let manager = clone_mgr(&self.manager);
        blocking::unblock(move || {
            let mut asset = manager
                .open(&resolved_path)
                .ok_or(std::io::ErrorKind::NotFound)?;
            let mut buf = Vec::with_capacity(asset.get_length());
            asset.read_to_end(&mut buf)?;
            Ok(buf)
        })
        .await
    }
}

impl AssetOpen for AndroidAssetLoaderIo {
    fn load(&self, asset_path: &AssetPath) -> BoxedFuture<'_, std::io::Result<Vec<u8>>> {
        let asset_path = asset_path.to_owned();
        Box::pin(async move { self.open(&asset_path).await })
    }
}
