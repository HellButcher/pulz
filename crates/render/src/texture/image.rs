use thiserror::Error;

use super::{TextureDescriptor, TextureDimensions, TextureFormat, TextureId};
use crate::{backend::RenderBackend, render_asset::RenderAsset};

use image::{buffer::ConvertBuffer, ImageFormat};

pub struct Image {
    pub data: Vec<u8>,
    pub descriptor: TextureDescriptor,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ImageError {
    #[error("Unable to detect image format")]
    UnknownFormat,

    #[error("unable to load image")]
    ImageError(#[from] image::ImageError),
}

impl<B: RenderBackend> RenderAsset<B> for Image {
    type Target = TextureId;

    fn prepare(&self, backend: &mut B) -> Self::Target {
        let texture = backend.create_texture(&self.descriptor);
        backend.write_image(texture, self);
        texture
    }
}

impl Image {
    fn from_image_buffer_with_format<P: image::Pixel<Subpixel = u8> + 'static>(
        format: TextureFormat,
        image: image::ImageBuffer<P, Vec<u8>>,
    ) -> Self {
        let dimensions = image.dimensions();

        let data = image.into_raw();
        Self {
            data,
            descriptor: TextureDescriptor {
                format,
                dimensions: TextureDimensions::D2(dimensions.into()),
                ..Default::default()
            },
        }
    }

    fn from_image_buffer_u16_with_format<P: image::Pixel<Subpixel = u16> + 'static>(
        format: TextureFormat,
        image: image::ImageBuffer<P, Vec<u16>>,
    ) -> Self {
        let dimensions = image.dimensions();

        // TODO: use `vec_into_raw_parts` and `from_raw_parts_in` when stabilized
        let data = image.into_raw();
        let bytes: &[u8] = bytemuck::cast_slice(&data);
        Self {
            data: Vec::from(bytes),
            descriptor: TextureDescriptor {
                format,
                dimensions: TextureDimensions::D2(dimensions.into()),
                ..Default::default()
            },
        }
    }

    pub fn load(
        file_ext: Option<&str>,
        mimetype: Option<&str>,
        data: &[u8],
    ) -> Result<Self, ImageError> {
        let format = mimetype
            .and_then(format_from_mimetype)
            .or_else(|| file_ext.and_then(ImageFormat::from_extension))
            .or_else(|| image::guess_format(data).ok())
            .ok_or(ImageError::UnknownFormat)?;
        let image = image::load_from_memory_with_format(data, format)?;
        Ok(image.into())
    }
}

fn format_from_mimetype(mime: &str) -> Option<ImageFormat> {
    // STRIP: ("image/"|"application/")(-x)?EXTENSION(:.*)
    let mut ext = mime
        .strip_prefix("image/")
        .or_else(|| mime.strip_prefix("application/"))?;
    if let Some(offset) = ext.find(':') {
        ext = &ext[0..offset];
    };
    match ext {
        // map special names
        "x-icon" | "vnd.microsoft.icon" => Some(ImageFormat::Ico),
        "vnd.radiance" => Some(ImageFormat::Hdr),
        "x-portable-bitmap" | "x-portable-pixmap" => Some(ImageFormat::Pnm),
        // map names by extension
        _ => {
            if let Some(rest) = ext.strip_prefix("x-") {
                ext = rest;
            }
            ImageFormat::from_extension(ext)
        }
    }
}

impl From<image::DynamicImage> for Image {
    fn from(image: image::DynamicImage) -> Self {
        use image::DynamicImage;
        match image {
            DynamicImage::ImageLuma8(buffer) => buffer.into(),
            DynamicImage::ImageLumaA8(buffer) => buffer.into(),
            DynamicImage::ImageRgb8(buffer) => buffer.into(),
            DynamicImage::ImageRgba8(buffer) => buffer.into(),
            DynamicImage::ImageBgr8(buffer) => buffer.into(),
            DynamicImage::ImageBgra8(buffer) => buffer.into(),
            DynamicImage::ImageLuma16(buffer) => buffer.into(),
            DynamicImage::ImageLumaA16(buffer) => buffer.into(),
            DynamicImage::ImageRgb16(buffer) => buffer.into(),
            DynamicImage::ImageRgba16(buffer) => buffer.into(),
        }
    }
}

impl From<image::ImageBuffer<image::Luma<u8>, Vec<u8>>> for Image {
    #[inline]
    fn from(image: image::ImageBuffer<image::Luma<u8>, Vec<u8>>) -> Self {
        Self::from_image_buffer_with_format(TextureFormat::R8Unorm, image)
    }
}

impl From<image::ImageBuffer<image::LumaA<u8>, Vec<u8>>> for Image {
    #[inline]
    fn from(image: image::ImageBuffer<image::LumaA<u8>, Vec<u8>>) -> Self {
        Self::from_image_buffer_with_format(TextureFormat::Rg8Unorm, image)
    }
}

impl From<image::ImageBuffer<image::Rgb<u8>, Vec<u8>>> for Image {
    #[inline]
    fn from(image: image::ImageBuffer<image::Rgb<u8>, Vec<u8>>) -> Self {
        let image_rgba: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> = image.convert();
        Self::from_image_buffer_with_format(TextureFormat::Rgba8Srgb, image_rgba)
    }
}

impl From<image::ImageBuffer<image::Bgr<u8>, Vec<u8>>> for Image {
    #[inline]
    fn from(image: image::ImageBuffer<image::Bgr<u8>, Vec<u8>>) -> Self {
        let image_bgra: image::ImageBuffer<image::Bgra<u8>, Vec<u8>> = image.convert();
        Self::from_image_buffer_with_format(TextureFormat::Bgra8Srgb, image_bgra)
    }
}

impl From<image::ImageBuffer<image::Rgba<u8>, Vec<u8>>> for Image {
    #[inline]
    fn from(image: image::ImageBuffer<image::Rgba<u8>, Vec<u8>>) -> Self {
        Self::from_image_buffer_with_format(TextureFormat::Rgba8Srgb, image)
    }
}

impl From<image::ImageBuffer<image::Bgra<u8>, Vec<u8>>> for Image {
    #[inline]
    fn from(image: image::ImageBuffer<image::Bgra<u8>, Vec<u8>>) -> Self {
        Self::from_image_buffer_with_format(TextureFormat::Bgra8Srgb, image)
    }
}

impl From<image::ImageBuffer<image::Luma<u16>, Vec<u16>>> for Image {
    #[inline]
    fn from(image: image::ImageBuffer<image::Luma<u16>, Vec<u16>>) -> Self {
        Self::from_image_buffer_u16_with_format(TextureFormat::R16Uint, image)
    }
}

impl From<image::ImageBuffer<image::LumaA<u16>, Vec<u16>>> for Image {
    #[inline]
    fn from(image: image::ImageBuffer<image::LumaA<u16>, Vec<u16>>) -> Self {
        Self::from_image_buffer_u16_with_format(TextureFormat::Rg16Uint, image)
    }
}

impl From<image::ImageBuffer<image::Rgb<u16>, Vec<u16>>> for Image {
    #[inline]
    fn from(image: image::ImageBuffer<image::Rgb<u16>, Vec<u16>>) -> Self {
        let image_rgba: image::ImageBuffer<image::Rgba<u16>, Vec<u16>> = image.convert();
        Self::from_image_buffer_u16_with_format(TextureFormat::Rgba16Uint, image_rgba)
    }
}

impl From<image::ImageBuffer<image::Rgba<u16>, Vec<u16>>> for Image {
    #[inline]
    fn from(image: image::ImageBuffer<image::Rgba<u16>, Vec<u16>>) -> Self {
        Self::from_image_buffer_u16_with_format(TextureFormat::Rgba16Uint, image)
    }
}
