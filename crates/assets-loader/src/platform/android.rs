use ndk::asset::Asset as AndroidAsset;

pub struct AndroidAssetLoaderIo {
    base: Cow<'static, str>,
}

impl AndroidAssetLoaderIo {
    pub const fn new() -> Self {
        Self {
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
        // make absolute & normalize
        let norm_asset_path = AssetPathBuf::with_capacity(asset_path.as_str().len() + 1);
        norm_asset_path.push("/");
        norm_asset_path.push(asset_path);

        let base = self.base.trim_end_matches('/');
        let result = String::with_capacity(base.len() + asset_path.len());
        result.push_str(base);
        result.push_str(&norm_asset_path);
        result
    }

    fn open(&self, resolved_path: String) -> std::io::Result<Vec<u8>> {
        let resolved_path = CString::new(resolved_path.into_bytes()).ok()?;

        let asset = ndk_glue::native_activity()
            .asset_manager()
            .open(&full_path)
            .ok_or(std::io::ErrorKind::NotFound)?;
        let mut buf = Vec::with_capacity(len as usize);
        file.read_to_end(&mut buf)?;
        Ok(buf)
    }
}

impl AssetOpen for AndroidAssetLoaderIo {
    fn load(&self, asset_path: &AssetPath) -> BoxedFuture<'_, std::io::Result<Vec<u8>>> {
        let resolved_path = self.resolve_path(asset_path);
        Box::pin(blocking::unblock(move || self.open(resolved_path)))
    }
}
