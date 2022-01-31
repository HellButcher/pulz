use image::{buffer::ConvertBuffer, ImageFormat};
use thiserror::Error;

use super::{ImageDataLayout, TextureDescriptor, TextureDimensions, TextureFormat};
use crate::math::uvec2;

pub struct Image {
    pub data: Vec<u8>,
    pub layout: ImageDataLayout,
    pub descriptor: TextureDescriptor,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ImageError {
    #[error("Unable to detect image format")]
    UnknownFormat,

    #[error("Unsupported image layout")]
    UnsupportedImageLayout,

    #[error("unable to load image")]
    ImageError(#[from] image::ImageError),
}

// impl<B: RenderBackend> RenderAsset<B> for Image {
//     type Target = Texture;

//     fn prepare(&self, backend: &mut B) -> Self::Target {
//         let texture = backend.create_texture(&self.descriptor).unwrap();
//         backend.write_image(&texture, self);
//         texture
//     }
// }

impl Image {
    fn from_image_buffer_with_format<P>(
        format: TextureFormat,
        image: image::ImageBuffer<P, Vec<P::Subpixel>>,
    ) -> Self
    where
        P: image::Pixel + 'static,
        P::Subpixel: bytemuck::Pod,
    {
        let (width, height) = image.dimensions();
        let bytes_per_pixel = std::mem::size_of::<P>();
        let bytes_per_row = bytes_per_pixel as u32 * width;
        let data = image.into_raw();
        let bytes: Vec<u8> = bytemuck::cast_vec(data);
        Self {
            data: bytes,
            layout: ImageDataLayout {
                offset: 0,
                bytes_per_row,
                rows_per_image: height,
            },
            descriptor: TextureDescriptor {
                format,
                dimensions: TextureDimensions::D2(uvec2(width, height)),
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
        image.try_into()
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

impl TryFrom<image::DynamicImage> for Image {
    type Error = ImageError;
    fn try_from(image: image::DynamicImage) -> Result<Self, ImageError> {
        use image::DynamicImage;
        Ok(match image {
            DynamicImage::ImageLuma8(buffer) => buffer.into(),
            DynamicImage::ImageLumaA8(buffer) => buffer.into(),
            DynamicImage::ImageRgb8(buffer) => buffer.into(),
            DynamicImage::ImageRgba8(buffer) => buffer.into(),
            DynamicImage::ImageLuma16(buffer) => buffer.into(),
            DynamicImage::ImageLumaA16(buffer) => buffer.into(),
            DynamicImage::ImageRgb16(buffer) => buffer.into(),
            DynamicImage::ImageRgba16(buffer) => buffer.into(),
            DynamicImage::ImageRgb32F(buffer) => buffer.into(),
            DynamicImage::ImageRgba32F(buffer) => buffer.into(),
            _ => {
                return Err(ImageError::UnsupportedImageLayout);
            }
        })
    }
}

macro_rules! impl_from_imagebuffer {
    ($(
        $imgform:ident <$prim:ty> $(as $imgconv:ident <$convprim:ty>)? => $texform:ident ;
    )*) => {$(
        impl From<image::ImageBuffer<image::$imgform<$prim>, Vec<$prim>>> for Image {
            #[inline]
            fn from(image: image::ImageBuffer<image::$imgform<$prim>, Vec<$prim>>) -> Self {
                $(
                    let image: image::ImageBuffer<image::$imgconv<$convprim>, Vec<$convprim>> = image.convert();
                )?
                Self::from_image_buffer_with_format(TextureFormat::$texform, image)
            }
        }
    )*};
}

impl_from_imagebuffer! {
    Luma<u8> => R8Unorm;
    LumaA<u8> => Rg8Unorm;
    Rgb<u8> as Rgba<u8> => Rgba8UnormSrgb;
    Rgba<u8> => Rgba8UnormSrgb;
    Luma<u16> as Luma<f32> => R32Float;
    LumaA<u16> as LumaA<f32> => Rg32Float;
    Rgb<u16> as Rgba<f32> => Rgba32Float;
    Rgba<u16> as Rgba<f32> => Rgba32Float;
    Luma<f32> => R32Float;
    LumaA<f32> => Rg32Float;
    Rgb<f32> as Rgba<f32> => Rgba32Float;
    Rgba<f32> => Rgba32Float;
}
