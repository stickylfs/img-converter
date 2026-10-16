mod common;

use common::*;
use img_converter_common::ImageFormat;
use img_converter_core::ImageEngine;
use std::fs;

#[test]
fn test_merge_images_into_pdf_and_extract_pages() {
    let dir = temp_dir("pdf");

    // Named so that natural ordering (page2 < page10) differs from plain sorting.
    let page2 = dir.join("page2.png");
    let page10 = dir.join("page10.png");
    let broken = dir.join("page3.png");
    write_solid_png(&page2, 64, 64, [255, 0, 0, 255]);
    write_solid_png(&page10, 96, 64, [0, 255, 0, 128]);
    fs::write(&broken, b"not an image").unwrap();

    // 1. Merge: one PDF with two pages, plus one reported failure.
    let summary = ImageEngine::convert_batch(&request(
        vec![page10.clone(), broken.clone(), page2.clone()],
        ImageFormat::Pdf,
    ));
    assert_eq!(summary.successful, 1, "{:?}", summary.results);
    assert_eq!(summary.failed, 1);
    let failure = summary.results.iter().find(|r| !r.is_success()).unwrap();
    assert_eq!(failure.input_path, broken);

    let pdf_path = summary
        .results
        .iter()
        .find_map(|r| r.output_path.clone())
        .unwrap();
    assert_eq!(pdf_path, dir.join("page2.pdf"));
    let doc = lopdf::Document::load(&pdf_path).unwrap();
    assert_eq!(doc.get_pages().len(), 2);

    // 2. Extract: pages go to a "page2" subfolder as page2_page_01.png, ...
    let summary = ImageEngine::convert_batch(&request(vec![pdf_path], ImageFormat::Png));
    assert_eq!(summary.successful, 1, "{:?}", summary.results);
    let pages_dir = dir.join("page2");
    assert!(pages_dir.join("page2_page_01.png").exists());
    assert!(pages_dir.join("page2_page_02.png").exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_single_image_to_pdf() {
    let dir = temp_dir("pdf_single");
    let png = dir.join("scan.png");
    write_transparent_png(&png, 300, 150);

    let summary = ImageEngine::convert_batch(&request(vec![png], ImageFormat::Pdf));
    assert_eq!(summary.successful, 1);
    let doc = lopdf::Document::load(dir.join("scan.pdf")).unwrap();
    assert_eq!(doc.get_pages().len(), 1);

    let _ = fs::remove_dir_all(&dir);
}
