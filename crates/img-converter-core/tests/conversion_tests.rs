mod common;

use common::*;
use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, ImageEncoder};
use img_converter_common::{CollisionPolicy, DecodeLimits, ImageFormat, WindowsCodec};
use img_converter_core::ImageEngine;
use std::fs;
use std::path::Path;

/// Decodes a converted file back with the engine to validate it.
fn decode_output(path: &Path) -> img_converter_core::DecodedImage {
    let bytes = fs::read(path).unwrap();
    let format = ImageEngine::detect_format(path, &bytes).unwrap();
    ImageEngine::decode_image(format, &bytes, &DecodeLimits::default(), Some(1.0)).unwrap()
}

#[test]
fn test_every_source_to_every_output() {
    let dir = temp_dir("matrix");
    let jpg = dir.join("sample_a.jpg");
    let png = dir.join("sample_b.png");
    let svg = dir.join("sample_c.svg");
    write_jpeg(&jpg, 32, 24);
    write_transparent_png(&png, 32, 24);
    write_svg(&svg);

    for source in [&jpg, &png, &svg] {
        for &target in ImageFormat::OUTPUT_FORMATS {
            let out_dir = dir.join(format!(
                "{}_{}",
                source.extension().unwrap().to_string_lossy(),
                target.extension()
            ));
            let mut request = request(vec![], target);
            request.output_dir = Some(out_dir);
            let res = ImageEngine::convert_single(source, &request);
            assert!(
                res.is_success(),
                "{} -> {} failed: {:?}",
                source.display(),
                target,
                res.error
            );

            let output = res.output_path.unwrap();
            if target == ImageFormat::Pdf {
                assert!(fs::read(&output).unwrap().starts_with(b"%PDF-"));
                continue;
            }
            // Reading AVIF needs the optional Windows AV1 codec (absent on CI runners).
            if target == ImageFormat::Avif && !WindowsCodec::Av1.is_installed() {
                assert_eq!(&fs::read(&output).unwrap()[4..8], b"ftyp");
                continue;
            }
            let decoded = decode_output(&output);
            let expected = if source == &svg {
                (64, 64) // default config renders SVG at 2x
            } else {
                (32, 24)
            };
            assert_eq!(
                (decoded.width, decoded.height),
                expected,
                "{} -> {} has wrong dimensions",
                source.display(),
                target
            );
        }
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_transparency_is_composited_on_white() {
    let dir = temp_dir("matte");
    let png = dir.join("transparent.png");
    write_solid_png(&png, 8, 8, [0, 0, 0, 0]);

    let res = ImageEngine::convert_single(&png, &request(vec![], ImageFormat::Bmp));
    let decoded = decode_output(res.output_path.as_ref().unwrap());
    assert!(!decoded.has_alpha());
    assert_eq!(&decoded.to_rgb8_raw()[..3], &[255, 255, 255]);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_exif_orientation_is_applied() {
    let dir = temp_dir("orientation");
    let jpg = dir.join("rotated.jpg");

    // 4x2 image tagged "rotate 90 CW" must come out as 2x4.
    let pixels = vec![128u8; 4 * 2 * 3];
    let mut bytes = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut bytes, 90);
    encoder.set_exif_metadata(exif_with_orientation(6)).unwrap();
    encoder
        .write_image(&pixels, 4, 2, ExtendedColorType::Rgb8)
        .unwrap();
    fs::write(&jpg, &bytes).unwrap();

    let res = ImageEngine::convert_single(&jpg, &request(vec![], ImageFormat::Png));
    assert!(res.is_success(), "{:?}", res.error);
    let output = res.output_path.unwrap();
    let decoded = decode_output(&output);
    assert_eq!((decoded.width, decoded.height), (2, 4));
    // The preserved EXIF must no longer ask viewers to rotate again.
    assert_eq!(decoded.orientation, 1);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_batch_name_collisions_are_resolved() {
    let dir = temp_dir("collisions");
    let jpg = dir.join("photo.jpg");
    let png = dir.join("photo.png");
    write_jpeg(&jpg, 16, 16);
    write_transparent_png(&png, 16, 16);
    fs::write(dir.join("photo.webp"), b"existing").unwrap();

    let summary = ImageEngine::convert_batch(&request(vec![jpg, png], ImageFormat::Webp));
    assert_eq!(summary.successful, 2, "{:?}", summary.results);

    // The existing file is untouched and both conversions got distinct names.
    assert_eq!(fs::read(dir.join("photo.webp")).unwrap(), b"existing");
    assert!(dir.join("photo (1).webp").exists());
    assert!(dir.join("photo (2).webp").exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_overwrite_policy() {
    let dir = temp_dir("overwrite");
    let jpg = dir.join("photo.jpg");
    write_jpeg(&jpg, 16, 16);
    fs::write(dir.join("photo.png"), b"existing").unwrap();

    let mut req = request(vec![jpg.clone()], ImageFormat::Png);
    req.collision = CollisionPolicy::Overwrite;
    let summary = ImageEngine::convert_batch(&req);
    assert_eq!(summary.successful, 1);
    assert!(fs::read(dir.join("photo.png"))
        .unwrap()
        .starts_with(b"\x89PNG"));
    assert!(!dir.join("photo (1).png").exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_jfif_to_jpg_is_a_lossless_copy() {
    let dir = temp_dir("jfif");
    let jfif = dir.join("download.jfif");
    write_jpeg(&jfif.with_extension("jpg"), 16, 16);
    fs::rename(jfif.with_extension("jpg"), &jfif).unwrap();

    let res = ImageEngine::convert_single(&jfif, &request(vec![], ImageFormat::Jpeg));
    assert!(res.is_success(), "{:?}", res.error);
    assert_eq!(
        fs::read(res.output_path.unwrap()).unwrap(),
        fs::read(&jfif).unwrap()
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_decode_limits_are_enforced() {
    let dir = temp_dir("limits");
    let png = dir.join("big.png");
    write_solid_png(&png, 64, 64, [10, 20, 30, 255]);

    let mut req = request(vec![], ImageFormat::Jpeg);
    req.limits.max_width = 32;
    let res = ImageEngine::convert_single(&png, &req);
    assert!(!res.is_success());
    assert!(res.error.unwrap().contains("limit"));

    let mut req = request(vec![], ImageFormat::Jpeg);
    req.limits.max_input_file_size = 10;
    assert!(!ImageEngine::convert_single(&png, &req).is_success());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_hdr_roundtrip_preserves_tones() {
    let dir = temp_dir("hdr");
    let png = dir.join("grey.png");
    write_solid_png(&png, 4, 4, [64, 128, 192, 255]);

    let hdr = ImageEngine::convert_single(&png, &request(vec![], ImageFormat::Hdr));
    let back = ImageEngine::convert_single(
        hdr.output_path.as_ref().unwrap(),
        &request(vec![], ImageFormat::Png),
    );
    let decoded = decode_output(back.output_path.as_ref().unwrap());
    let px = &decoded.to_rgb8_raw()[..3];
    // RGBE has limited precision, but sRGB <-> linear must not shift mid-tones.
    for (got, want) in px.iter().zip([64u8, 128, 192]) {
        assert!(got.abs_diff(want) <= 3, "got {:?}", px);
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_unsupported_and_missing_files_fail_cleanly() {
    let dir = temp_dir("invalid");
    let text = dir.join("notes.txt");
    fs::write(&text, b"just text").unwrap();
    let corrupt = dir.join("broken.png");
    fs::write(&corrupt, b"\x89PNG\r\n\x1a\nnot really").unwrap();

    let summary = ImageEngine::convert_batch(&request(
        vec![text, corrupt, dir.join("missing.png")],
        ImageFormat::Jpeg,
    ));
    assert_eq!(summary.failed, 3);
    assert_eq!(
        summary.exit_code(),
        img_converter_common::ExitCode::DECODE_ERROR
    );
    // Failed conversions leave no partial or placeholder files behind.
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_raw_embedded_preview_fallback() {
    let dir = temp_dir("raw");
    let preview = dir.join("preview.jpg");
    write_jpeg(&preview, 1000, 1000);

    // Mock CR3 container: ftyp box followed by an embedded full-size JPEG preview.
    let mut cr3 = vec![
        0x00, 0x00, 0x00, 0x18, b'f', b't', b'y', b'p', b'c', b'r', b'x', b' ', 0x00, 0x00, 0x00,
        0x00, b'c', b'r', b'x', b' ', b'i', b's', b'o', b'm',
    ];
    cr3.extend_from_slice(&fs::read(&preview).unwrap());
    let raw = dir.join("camera.cr3");
    fs::write(&raw, &cr3).unwrap();

    let res = ImageEngine::convert_single(&raw, &request(vec![], ImageFormat::Png));
    assert!(res.is_success(), "{:?}", res.error);
    let decoded = decode_output(res.output_path.as_ref().unwrap());
    assert_eq!((decoded.width, decoded.height), (1000, 1000));

    let _ = fs::remove_dir_all(&dir);
}
