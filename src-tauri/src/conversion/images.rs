use super::{check_cancel, parse_pages, ConversionOptions, INPUT_LIMIT, OUTPUT_LIMIT, PIXEL_LIMIT};
use image::{AnimationDecoder, DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use std::{
    fs::{self, File},
    io::{BufWriter, Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Instant,
};

fn image_error(_: impl std::fmt::Display) -> String {
    "Conversion invalid or unsupported image".into()
}

fn limits() -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(PIXEL_LIMIT as u32);
    limits.max_image_height = Some(PIXEL_LIMIT as u32);
    limits.max_alloc = Some(512 * 1024 * 1024);
    limits
}

fn dimensions(width: u32, height: u32) -> Result<(), String> {
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > PIXEL_LIMIT {
        Err("Conversion image pixel size limit exceeded".into())
    } else {
        Ok(())
    }
}

fn frame_selection(options: &ConversionOptions) -> Result<Option<usize>, String> {
    let pages = parse_pages(&options.pages)?;
    if pages.len() > 1 {
        return Err("Conversion select one image frame or page".into());
    }
    Ok(pages.first().map(|page| (*page - 1) as usize))
}

fn animation<'a>(
    frames: image::Frames<'a>,
    selection: Option<usize>,
    cancel: &AtomicBool,
) -> Result<DynamicImage, String> {
    let begin = Instant::now();
    let mut frames = frames;
    let mut selected = None;
    let wanted = selection.unwrap_or(0);
    for index in 0..=wanted.max(1) {
        check_cancel(cancel)?;
        if begin.elapsed().as_secs() > 120 {
            return Err("Conversion image timed out".into());
        }
        let frame = match frames.next() {
            Some(frame) => frame.map_err(image_error)?,
            None => break,
        };
        dimensions(frame.buffer().width(), frame.buffer().height())?;
        if index == 1 && selection.is_none() {
            return Err("Conversion multiframe image requires an explicit frame".into());
        }
        if index == wanted {
            selected = Some(DynamicImage::ImageRgba8(frame.into_buffer()));
            if selection.is_some() {
                break;
            }
        }
    }
    selected.ok_or("Conversion image frame is unavailable".into())
}

fn decode(
    mut bytes: Vec<u8>,
    format: ImageFormat,
    options: &ConversionOptions,
    cancel: &AtomicBool,
) -> Result<DynamicImage, String> {
    let selection = frame_selection(options)?;
    match format {
        ImageFormat::Gif => {
            let mut decoder =
                image::codecs::gif::GifDecoder::new(Cursor::new(bytes)).map_err(image_error)?;
            let (width, height) = decoder.dimensions();
            dimensions(width, height)?;
            decoder.set_limits(limits()).map_err(image_error)?;
            return animation(decoder.into_frames(), selection, cancel);
        }
        ImageFormat::WebP => {
            let mut decoder =
                image::codecs::webp::WebPDecoder::new(Cursor::new(bytes)).map_err(image_error)?;
            let (width, height) = decoder.dimensions();
            dimensions(width, height)?;
            decoder.set_limits(limits()).map_err(image_error)?;
            if decoder.has_animation() {
                if selection.is_none() {
                    return Err("Conversion multiframe image requires an explicit frame".into());
                }
                return animation(decoder.into_frames(), selection, cancel);
            }
            if selection.is_some_and(|page| page != 0) {
                return Err("Conversion image frame is unavailable".into());
            }
            let orientation = decoder.orientation().map_err(image_error)?;
            let mut image = DynamicImage::from_decoder(decoder).map_err(image_error)?;
            image.apply_orientation(orientation);
            return Ok(image);
        }
        ImageFormat::Png => {
            let mut decoder =
                image::codecs::png::PngDecoder::with_limits(Cursor::new(bytes), limits())
                    .map_err(image_error)?;
            let (width, height) = decoder.dimensions();
            dimensions(width, height)?;
            decoder.set_limits(limits()).map_err(image_error)?;
            if decoder.is_apng().map_err(image_error)? {
                if selection.is_none() {
                    return Err("Conversion multiframe image requires an explicit frame".into());
                }
                return animation(
                    decoder.apng().map_err(image_error)?.into_frames(),
                    selection,
                    cancel,
                );
            }
            if selection.is_some_and(|page| page != 0) {
                return Err("Conversion image frame is unavailable".into());
            }
            let orientation = decoder.orientation().map_err(image_error)?;
            let mut image = DynamicImage::from_decoder(decoder).map_err(image_error)?;
            image.apply_orientation(orientation);
            return Ok(image);
        }
        ImageFormat::Tiff => select_tiff_page(&mut bytes, selection)?,
        _ => {
            if selection.is_some_and(|page| page != 0) {
                return Err("Conversion image frame is unavailable".into());
            }
        }
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits());
    let mut decoder = reader.into_decoder().map_err(image_error)?;
    let (width, height) = decoder.dimensions();
    dimensions(width, height)?;
    let orientation = decoder.orientation().map_err(image_error)?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(image_error)?;
    image.apply_orientation(orientation);
    Ok(image)
}

// 仅重定向副本中首个 IFD 的指针，复用 image 的有界 TIFF 解码器读取选定页面。
fn select_tiff_page(bytes: &mut [u8], selection: Option<usize>) -> Result<(), String> {
    if bytes.len() < 8 {
        return Err("Conversion invalid TIFF".into());
    }
    let little = match &bytes[..2] {
        b"II" => true,
        b"MM" => false,
        _ => return Err("Conversion invalid TIFF".into()),
    };
    fn number(bytes: &[u8], offset: usize, size: usize, little: bool) -> Result<u64, String> {
        let data = bytes
            .get(offset..offset.checked_add(size).ok_or("Conversion invalid TIFF")?)
            .ok_or("Conversion invalid TIFF")?;
        let mut value = 0u64;
        for (index, byte) in data.iter().enumerate() {
            value |= u64::from(*byte) << ((if little { index } else { size - 1 - index }) * 8);
        }
        Ok(value)
    }
    let kind = number(bytes, 2, 2, little)?;
    let (header, count_size, entry_size, next_size) = match kind {
        42 => (4, 2, 12, 4),
        43 => {
            if number(bytes, 4, 2, little)? != 8 || number(bytes, 6, 2, little)? != 0 {
                return Err("Conversion invalid TIFF".into());
            }
            (8, 8, 20, 8)
        }
        _ => return Err("Conversion invalid TIFF".into()),
    };
    let mut offset = number(bytes, header, next_size, little)?;
    let mut pages = Vec::new();
    let mut seen = std::collections::HashSet::new();
    while offset != 0 {
        if pages.len() >= 500 || !seen.insert(offset) {
            return Err("Conversion TIFF page size limit exceeded".into());
        }
        let position = usize::try_from(offset).map_err(|_| "Conversion invalid TIFF")?;
        let count = usize::try_from(number(bytes, position, count_size, little)?)
            .map_err(|_| "Conversion invalid TIFF")?;
        let next = position
            .checked_add(count_size)
            .and_then(|value| {
                count
                    .checked_mul(entry_size)
                    .and_then(|length| value.checked_add(length))
            })
            .ok_or("Conversion invalid TIFF")?;
        pages.push(offset);
        offset = number(bytes, next, next_size, little)?;
    }
    if pages.len() > 1 && selection.is_none() {
        return Err("Conversion multiframe TIFF requires an explicit frame or page".into());
    }
    let offset = *pages
        .get(selection.unwrap_or(0))
        .ok_or("Conversion TIFF page unavailable")?;
    let target = bytes
        .get_mut(header..header + next_size)
        .ok_or("Conversion invalid TIFF")?;
    for (index, byte) in target.iter_mut().enumerate() {
        *byte = (offset >> ((if little { index } else { next_size - 1 - index }) * 8)) as u8;
    }
    Ok(())
}

pub(super) fn convert(
    source: &Path,
    directory: &Path,
    options: &ConversionOptions,
    cancel: &AtomicBool,
) -> Result<Vec<PathBuf>, String> {
    options.validate()?;
    check_cancel(cancel)?;
    let file = File::open(source).map_err(|_| "Conversion image unavailable")?;
    if file
        .metadata()
        .map_err(|_| "Conversion image unavailable")?
        .len()
        > INPUT_LIMIT
    {
        return Err("Conversion image size limit exceeded".into());
    }
    let mut bytes = Vec::new();
    file.take(INPUT_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Conversion image unavailable")?;
    if bytes.len() as u64 > INPUT_LIMIT {
        return Err("Conversion image size limit exceeded".into());
    }
    let format = image::guess_format(&bytes).map_err(image_error)?;
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .ok_or("Conversion unsupported image format")?;
    if ImageFormat::from_extension(extension) != Some(format) {
        return Err("Conversion image content does not match extension".into());
    }
    let mut image = decode(bytes, format, options, cancel)?;
    check_cancel(cancel)?;
    if options.target == "jpg" || options.target == "bmp" {
        let rgba = image.to_rgba8();
        let mut rgb = image::RgbImage::new(rgba.width(), rgba.height());
        for (input, output) in rgba.pixels().zip(rgb.pixels_mut()) {
            let alpha = u32::from(input.0[3]);
            for channel in 0..3 {
                output.0[channel] =
                    ((u32::from(input.0[channel]) * alpha + 255 * (255 - alpha) + 127) / 255) as u8;
            }
        }
        image = DynamicImage::ImageRgb8(rgb);
    }
    if options.target == "ico" {
        let size = u32::from(options.ico_size);
        let scaled = image
            .resize(size, size, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        let mut square = image::RgbaImage::new(size, size);
        image::imageops::overlay(
            &mut square,
            &scaled,
            i64::from((size - scaled.width()) / 2),
            i64::from((size - scaled.height()) / 2),
        );
        image = DynamicImage::ImageRgba8(square);
    }
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or("Conversion invalid image name")?;
    let output = directory.join(format!("{stem}.{}", options.target));
    // 同格式转换也只写入任务的独立输出目录，永不打开源文件进行写入。
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .map_err(|_| "Conversion image output unavailable")?;
    let mut writer = BufWriter::new(file);
    let result = (|| {
        match options.target.as_str() {
            "jpg" => {
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, options.quality)
                    .encode_image(&image)
                    .map_err(image_error)?
            }
            "gif" => image::codecs::gif::GifEncoder::new_with_speed(&mut writer, 10)
                .encode_frame(image::Frame::new(image.to_rgba8()))
                .map_err(image_error)?,
            "png" | "webp" | "bmp" | "tiff" | "ico" => image
                .write_to(
                    &mut writer,
                    ImageFormat::from_extension(&options.target)
                        .ok_or("Conversion unsupported image target")?,
                )
                .map_err(image_error)?,
            _ => return Err("Conversion unsupported image target".into()),
        }
        writer
            .flush()
            .map_err(|_| "Conversion image output unavailable")?;
        check_cancel(cancel)?;
        let metadata = fs::metadata(&output).map_err(|_| "Conversion image output unavailable")?;
        if metadata.len() == 0 || metadata.len() > OUTPUT_LIMIT {
            return Err("Conversion image output size limit exceeded".into());
        }
        let mut verify = ImageReader::open(&output).map_err(image_error)?;
        verify.limits(limits());
        let verified = verify.decode().map_err(image_error)?;
        if verified.width() != image.width() || verified.height() != image.height() {
            return Err("Conversion image output validation failed".into());
        }
        Ok(vec![output.clone()])
    })();
    drop(writer);
    if result.is_err() {
        let _ = fs::remove_file(output);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversion::ConversionOptions;
    use std::{fs, path::PathBuf, sync::atomic::AtomicBool};

    fn directory() -> PathBuf {
        let path = std::env::current_dir()
            .unwrap()
            .join("../test-results")
            .join(format!(
                "conversion-image-{}",
                crate::conversion::unique_id()
            ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn animated_gif_requires_explicit_frame_and_respects_selection() {
        let root = directory();
        let source = root.join("动画.gif");
        let mut encoder = image::codecs::gif::GifEncoder::new(fs::File::create(&source).unwrap());
        encoder
            .encode_frame(image::Frame::new(image::RgbaImage::from_pixel(
                2,
                2,
                image::Rgba([255, 0, 0, 255]),
            )))
            .unwrap();
        encoder
            .encode_frame(image::Frame::new(image::RgbaImage::from_pixel(
                2,
                2,
                image::Rgba([0, 255, 0, 255]),
            )))
            .unwrap();
        drop(encoder);
        let output = root.join("out");
        fs::create_dir(&output).unwrap();
        let mut options = ConversionOptions::default();
        let cancel = AtomicBool::new(false);
        assert!(convert(&source, &output, &options, &cancel)
            .unwrap_err()
            .contains("frame"));
        options.pages = "2".into();
        let result = convert(&source, &output, &options, &cancel).unwrap();
        assert_eq!(
            image::open(&result[0])
                .unwrap()
                .to_rgba8()
                .get_pixel(0, 0)
                .0,
            [0, 255, 0, 255]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn validates_content_signature_instead_of_trusting_extension() {
        let root = directory();
        let source = root.join("fake.jpg");
        image::RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255]))
            .save_with_format(&source, image::ImageFormat::Png)
            .unwrap();
        assert!(convert(
            &source,
            &root,
            &ConversionOptions::default(),
            &AtomicBool::new(false)
        )
        .is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn icon_output_is_a_real_icon_at_requested_size() {
        let root = directory();
        let source = root.join("wide.png");
        image::RgbaImage::from_pixel(30, 10, image::Rgba([15, 80, 160, 255]))
            .save(&source)
            .unwrap();
        let options = ConversionOptions {
            target: "ico".into(),
            ico_size: 64,
            ..Default::default()
        };
        let paths = convert(&source, &root, &options, &AtomicBool::new(false)).unwrap();
        assert_eq!(&fs::read(&paths[0]).unwrap()[..4], &[0, 0, 1, 0]);
        let decoded = image::open(&paths[0]).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (64, 64));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn jpeg_exif_orientation_is_applied_before_encoding() {
        let root = directory();
        let source = root.join("orientation.jpg");
        image::RgbImage::from_pixel(3, 2, image::Rgb([35, 70, 100]))
            .save(&source)
            .unwrap();
        let jpeg = fs::read(&source).unwrap();
        let exif = [
            b'E', b'x', b'i', b'f', 0, 0, b'I', b'I', 42, 0, 8, 0, 0, 0, 1, 0, 0x12, 1, 3, 0, 1, 0,
            0, 0, 6, 0, 0, 0, 0, 0, 0, 0,
        ];
        let mut rotated = vec![0xff, 0xd8, 0xff, 0xe1, 0, 34];
        rotated.extend_from_slice(&exif);
        rotated.extend_from_slice(&jpeg[2..]);
        fs::write(&source, rotated).unwrap();
        let result = convert(
            &source,
            &root,
            &ConversionOptions::default(),
            &AtomicBool::new(false),
        )
        .unwrap();
        let image = image::open(&result[0]).unwrap();
        assert_eq!((image.width(), image.height()), (2, 3));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn tiff_ifd_selection_checks_bounds_cycles_and_requires_page_choice() {
        // 两个空 IFD 仅用于验证目录选择；像素解码仍由成熟解码器执行。
        let bytes = vec![
            b'I', b'I', 42, 0, 8, 0, 0, 0, 0, 0, 14, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert!(select_tiff_page(&mut bytes.clone(), None)
            .unwrap_err()
            .contains("explicit"));
        let mut selected = bytes.clone();
        select_tiff_page(&mut selected, Some(1)).unwrap();
        assert_eq!(&selected[4..8], &[14, 0, 0, 0]);
        let mut cycle = bytes;
        cycle[16] = 8;
        assert!(select_tiff_page(&mut cycle, Some(0)).is_err());
        assert!(select_tiff_page(&mut [b'I', b'I', 42, 0, 255, 255, 255, 255], Some(0)).is_err());
    }

    #[test]
    fn every_exposed_static_image_target_has_a_real_decode_roundtrip() {
        let root = directory();
        let source = root.join("source.png");
        image::RgbaImage::from_pixel(11, 7, image::Rgba([30, 90, 140, 190]))
            .save(&source)
            .unwrap();
        for target in ["png", "jpg", "webp", "bmp", "tiff", "gif", "ico"] {
            let destination = root.join(target);
            fs::create_dir(&destination).unwrap();
            let options = ConversionOptions {
                target: target.into(),
                ico_size: 32,
                ..Default::default()
            };
            let output = convert(&source, &destination, &options, &AtomicBool::new(false)).unwrap();
            let decoded = image::open(&output[0]).unwrap();
            let expected = if target == "ico" { (32, 32) } else { (11, 7) };
            assert_eq!((decoded.width(), decoded.height()), expected, "{target}");
            let back = destination.join("back");
            fs::create_dir(&back).unwrap();
            let result = convert(
                &output[0],
                &back,
                &ConversionOptions::default(),
                &AtomicBool::new(false),
            )
            .unwrap();
            let decoded = image::open(&result[0]).unwrap();
            assert_eq!((decoded.width(), decoded.height()), expected, "{target}");
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn png_dimensions_are_rejected_before_decompression_allocates_full_image() {
        let root = directory();
        let source = root.join("large.png");
        image::RgbaImage::from_pixel(1, 1, image::Rgba([1, 2, 3, 255]))
            .save(&source)
            .unwrap();
        let mut bytes = fs::read(&source).unwrap();
        bytes[16..20].copy_from_slice(&10000u32.to_be_bytes());
        bytes[20..24].copy_from_slice(&10000u32.to_be_bytes());
        let mut crc = 0xffff_ffffu32;
        for byte in &bytes[12..29] {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ if crc & 1 == 1 { 0xedb8_8320 } else { 0 };
            }
        }
        bytes[29..33].copy_from_slice(&(!crc).to_be_bytes());
        fs::write(&source, bytes).unwrap();
        let destination = root.join("out");
        fs::create_dir(&destination).unwrap();
        let error = convert(
            &source,
            &destination,
            &ConversionOptions::default(),
            &AtomicBool::new(false),
        )
        .unwrap_err();
        assert!(error.contains("pixel"), "{error}");
        assert!(fs::read_dir(destination).unwrap().next().is_none());
        fs::remove_dir_all(root).unwrap();
    }
}
