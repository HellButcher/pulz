use std::io::Cursor;

use js_sys::Uint8Array;
use wasm_bindgen_futures::JsFuture;

pub struct WasmFetchAssetLoaderIo {
    base: Cow<'static, str>,
}

impl WasmFetchAssetLoaderIo {
    pub const fn new() -> Self {
        Self {
            base: Cow::Borrowed("."),
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

    fn resolve_url(&self, asset_path: &AssetPath) -> String {
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

    async fn open(&self, resolved_url: &str) -> std::io::Result<Vec<u8>> {
        let window = web_sys::window().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "window not available")
        })?;
        let resp: Response = JsFuture::from(window.fetch_with_str(resolved_url))
            .await?
            .dyn_into()?;
        let buffer = JsFuture::from(resp.array_buffer()?).await?;
        let data = Uint8Array::new(&buffer).to_vec();
        Ok(data)
    }
}

impl AssetOpen for WasmFetchAssetLoaderIo {
    fn open(&self, asset_path: AssetPathBuf) -> BoxedFuture<'_, std::io::Result<Vec<u8>>> {
        let resolved_url = self.resolve_url(asset_path);
        Box::pin(blocking::unblock(move || self.open(&resolved_url)))
    }
}
