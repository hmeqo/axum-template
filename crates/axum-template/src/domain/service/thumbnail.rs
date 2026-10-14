use std::io::Cursor;

use image::{
    DynamicImage, ExtendedColorType, ImageDecoder, ImageEncoder, ImageReader, Limits, Rgba,
    RgbaImage, imageops::FilterType, metadata::Orientation,
};

use crate::{
    domain::vo::{ThumbFormat, ThumbSize},
    error::{AppError, Result},
};

/// 解码尺寸上限（严格限制）：源文件已被 `storage.max-file-size` 限住，但解码后的像素不受它约束。
const MAX_SOURCE_SIDE: u32 = 8192;
/// 单次解码的分配上限。
const MAX_DECODE_ALLOC: u64 = 128 * 1024 * 1024;
/// `inside` 缩放的重采样滤波器。
const FILTER: FilterType = FilterType::Lanczos3;
/// JPEG 兜底档质量。
const JPEG_QUALITY: u8 = 82;
/// AVIF 速度/质量：实测 256² 约 34 ms、比 JPEG q82 小 3 倍。
const AVIF_SPEED: u8 = 10;
const AVIF_QUALITY: u8 = 60;

/// 可生成派生图的栅格类型。
// SVG 不在内（内联渲染即存储型 XSS），GIF 不在内（首帧语义含混）。
pub fn is_thumbnailable(mime: &str) -> bool {
    matches!(
        mime,
        "image/jpeg" | "image/jpg" | "image/png" | "image/webp"
    )
}

/// 只读图片头，填 `files.width/height` 用；按 EXIF 方向给出**显示**尺寸。打不开即 `None`（无关错误不上抛）。
pub fn probe_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut decoder = decoder_of(bytes)?;
    let (width, height) = decoder.dimensions();
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);

    Some(if swaps_axes(orientation) {
        (height, width)
    } else {
        (width, height)
    })
}

/// `size` 是**最长边上限**：保比例、不裁、永不放大；JPEG 档有 alpha 时先合成到白底。
pub fn render(bytes: &[u8], size: ThumbSize, format: ThumbFormat) -> Result<Vec<u8>> {
    let mut decoder = decoder_of(bytes).ok_or_else(unreadable)?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder).map_err(|_| unreadable())?;
    img.apply_orientation(orientation);

    let longest = img.width().max(img.height());
    let target = size.get().min(longest);
    if target < longest {
        // `resize` 是 fit-inside（保比例、取最大可放入尺寸）；target 已封顶到原图最长边，故不会放大。
        img = img.resize(target, target, FILTER);
    }

    match format {
        ThumbFormat::Avif => {
            let rgba = img.to_rgba8();
            let mut buf = Vec::new();
            image::codecs::avif::AvifEncoder::new_with_speed_quality(
                &mut buf,
                AVIF_SPEED,
                AVIF_QUALITY,
            )
            // 并发归 `FileService` 的信号量管，rav1e 自己的线程池关掉（实测单线程无额外代价）。
            .with_num_threads(Some(1))
            .write_image(
                rgba.as_raw(),
                rgba.width(),
                rgba.height(),
                ExtendedColorType::Rgba8,
            )
            .map_err(|_| unreadable())?;
            Ok(buf)
        }
        ThumbFormat::Jpeg => {
            let mut buf = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, JPEG_QUALITY)
                .encode_image(&DynamicImage::ImageRgb8(flatten_on_white(img)))
                .map_err(|_| unreadable())?;
            Ok(buf)
        }
    }
}

/// JPEG 没有 alpha：有 alpha 通道时先合成到白底（各家管线一致的白底默认）。
fn flatten_on_white(img: DynamicImage) -> image::RgbImage {
    if !img.color().has_alpha() {
        return img.to_rgb8();
    }

    let mut canvas = RgbaImage::from_pixel(img.width(), img.height(), Rgba([255, 255, 255, 255]));
    image::imageops::overlay(&mut canvas, &img.to_rgba8(), 0, 0);
    DynamicImage::ImageRgba8(canvas).to_rgb8()
}

fn decoder_of(bytes: &[u8]) -> Option<impl ImageDecoder + '_> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    reader.limits(limits());
    reader.into_decoder().ok()
}

// `Limits` 是 `#[non_exhaustive]`：只能先取 `default()` 再赋字段。
fn limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_SIDE);
    limits.max_image_height = Some(MAX_SOURCE_SIDE);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    limits
}

fn swaps_axes(orientation: Orientation) -> bool {
    matches!(
        orientation,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    )
}

/// 解不开 / 超过解码上限 / 编码失败：对客户端都是「这个文件给不出缩略图」，一律无 code 的 400。
fn unreadable() -> AppError {
    AppError::malformed_request("File is not a readable image")
}

#[cfg(test)]
mod tests {
    use image::{ImageFormat, Rgb, RgbImage};

    use super::*;
    use crate::domain::vo::ThumbSize;

    fn size(value: u32) -> ThumbSize {
        ThumbSize::parse(value).expect("a canonical size")
    }

    fn png(width: u32, height: u32, pixel: impl Fn(u32, u32) -> Rgba<u8>) -> Vec<u8> {
        let img = RgbaImage::from_fn(width, height, pixel);
        let mut buf = Vec::new();
        DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .expect("png encodes");
        buf
    }

    /// 26 字节 TIFF 块：`II*\0` + IFD 偏移 8(le) + 条目数 1(le) + tag 0x0112 + type 3 + count 1 + value + next 0。
    /// `JpegEncoder` 写 APP1 时会自己补 `Exif\0\0`。
    fn tiff_orientation(value: u16) -> Vec<u8> {
        let mut exif = Vec::new();
        exif.extend_from_slice(b"II*\0");
        exif.extend_from_slice(&8u32.to_le_bytes());
        exif.extend_from_slice(&1u16.to_le_bytes());
        exif.extend_from_slice(&0x0112u16.to_le_bytes());
        exif.extend_from_slice(&3u16.to_le_bytes());
        exif.extend_from_slice(&1u32.to_le_bytes());
        exif.extend_from_slice(&(value as u32).to_le_bytes());
        exif.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(exif.len(), 26);
        exif
    }

    fn jpeg_with_orientation(width: u32, height: u32, value: u16) -> Vec<u8> {
        let img = RgbImage::from_fn(width, height, |x, _| {
            if x < width / 2 {
                Rgb([255, 0, 0])
            } else {
                Rgb([0, 0, 255])
            }
        });
        let mut buf = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 90);
        encoder
            .set_exif_metadata(tiff_orientation(value))
            .expect("jpeg accepts exif");
        encoder
            .encode_image(&DynamicImage::ImageRgb8(img))
            .expect("jpeg encodes");
        buf
    }

    fn decode(bytes: &[u8]) -> DynamicImage {
        image::load_from_memory(bytes).expect("the derivative decodes")
    }

    /// AVIF 容器里 `ispe`（ImageSpatialExtentsProperty）的尺寸：`ispe` + version/flags(4) + width(be32) + height(be32)。
    // `image` 的 `avif` 特性只带编码器（解码要 `avif-native`，即 dav1d），所以测试从容器读尺寸而不是解码像素。
    fn avif_dimensions(bytes: &[u8]) -> (u32, u32) {
        assert_eq!(&bytes[4..12], b"ftypavif", "an AVIF container");
        let at = find(bytes, b"ispe").expect("ispe box");
        let fields = &bytes[at + 4..at + 16];

        (
            u32::from_be_bytes(fields[4..8].try_into().expect("width")),
            u32::from_be_bytes(fields[8..12].try_into().expect("height")),
        )
    }

    /// 有 alpha 的 AVIF 带辅助 alpha 层：`auxl` 项 + 规范定义的辅助类型 URN。
    fn avif_keeps_alpha(bytes: &[u8]) -> bool {
        find(bytes, b"auxl").is_some()
            && find(bytes, b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha").is_some()
    }

    fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    #[test]
    fn inside_keeps_aspect_and_never_enlarges() {
        let shrunk = render(
            &png(400, 300, |_, _| Rgba([10, 20, 30, 255])),
            size(64),
            ThumbFormat::Avif,
        )
        .unwrap();
        assert_eq!(avif_dimensions(&shrunk), (64, 48));

        let small = render(
            &jpeg_with_orientation(8, 4, 1),
            size(512),
            ThumbFormat::Avif,
        )
        .unwrap();
        assert_eq!(
            avif_dimensions(&small),
            (8, 4),
            "no upscale past the source"
        );

        let jpeg = render(
            &png(400, 300, |_, _| Rgba([10, 20, 30, 255])),
            size(64),
            ThumbFormat::Jpeg,
        )
        .unwrap();
        let jpeg = decode(&jpeg);
        assert_eq!((jpeg.width(), jpeg.height()), (64, 48));
    }

    #[test]
    fn orientation_is_applied_before_scaling() {
        let rotated = jpeg_with_orientation(8, 4, 6);

        assert_eq!(probe_dimensions(&rotated), Some((4, 8)), "display size");
        assert_eq!(
            probe_dimensions(&jpeg_with_orientation(8, 4, 1)),
            Some((8, 4))
        );

        let thumb = render(&rotated, size(64), ThumbFormat::Avif).unwrap();
        assert_eq!(avif_dimensions(&thumb), (4, 8));
    }

    #[test]
    fn a_transparent_png_flattens_onto_white_for_jpeg() {
        let transparent = png(2, 2, |_, _| Rgba([0, 0, 0, 0]));

        let jpeg = decode(&render(&transparent, size(64), ThumbFormat::Jpeg).unwrap());
        assert!(!jpeg.color().has_alpha());
        for pixel in jpeg.to_rgb8().pixels() {
            assert_eq!(
                *pixel,
                Rgb([255, 255, 255]),
                "transparent pixels become white"
            );
        }

        let avif = render(&transparent, size(64), ThumbFormat::Avif).unwrap();
        assert!(avif_keeps_alpha(&avif), "avif keeps the alpha layer");
        assert_eq!(avif_dimensions(&avif), (2, 2));

        let opaque = render(
            &png(2, 2, |_, _| Rgba([0, 0, 0, 255])),
            size(64),
            ThumbFormat::Avif,
        )
        .unwrap();
        assert!(
            !avif_keeps_alpha(&opaque),
            "an opaque image carries no alpha layer"
        );
    }

    #[test]
    fn garbage_bytes_are_rejected() {
        assert!(render(b"not an image", size(64), ThumbFormat::Avif).is_err());
        assert!(render(&[], size(64), ThumbFormat::Jpeg).is_err());
        assert_eq!(probe_dimensions(b"not an image"), None);
    }

    #[test]
    fn only_raster_formats_are_thumbnailable() {
        for mime in ["image/jpeg", "image/jpg", "image/png", "image/webp"] {
            assert!(is_thumbnailable(mime), "{mime}");
        }

        for mime in [
            "text/plain",
            "image/svg+xml",
            "image/gif",
            "application/pdf",
        ] {
            assert!(!is_thumbnailable(mime), "{mime}");
        }
    }
}
