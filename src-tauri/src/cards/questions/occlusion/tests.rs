use image::{codecs::jpeg::JpegEncoder, codecs::webp::WebPEncoder};
use serde_json::json;

use super::*;

fn id(value: u32) -> String {
    format!("{value:032x}")
}

fn rich(text: &str) -> serde_json::Value {
    json!({"type":"doc","content":[{"type":"paragraph","content":[
        {"type":"text","text":text}]}]})
}

fn question(mode: &str) -> ValidatedQuestion {
    ValidatedQuestion::parse(
        &json!({
            "schemaVersion":1,
            "question":{
                "kind":"image_occlusion","imageAssetId":id(1),
                "masks":[
                    {"id":id(2),"rect":{"x":0.1,"y":0.1,"width":0.2,"height":0.2}},
                    {"id":id(3),"rect":{"x":0.6,"y":0.6,"width":0.2,"height":0.2}}
                ],
                "questions":[{"id":id(4),"prompt":rich("Identify the region."),
                    "answer":rich("Private answer"),"explanation":null,
                    "maskIds":[id(2)],"mode":mode}]
            }
        })
        .to_string(),
    )
    .unwrap()
}

fn source() -> RgbaImage {
    RgbaImage::from_pixel(100, 100, Rgba([231, 117, 43, 255]))
}

fn image(pixels: &RgbaImage) -> CanonicalImage {
    CanonicalImage::decode(&encode_png(pixels).unwrap()).unwrap()
}

fn render(pixels: &RgbaImage, mode: &str, phase: RenderPhase) -> RgbaImage {
    let rendered = image(pixels)
        .render(&question(mode), &id(4), phase, 100)
        .unwrap();
    CanonicalImage::decode(&rendered.png).unwrap().pixels
}

#[test]
fn canonical_png_round_trip_is_upright_and_lossless() {
    let pixels = source();
    let image = image(&pixels);
    assert_eq!(image.dimensions(), (100, 100));
    let reopened = CanonicalImage::decode(&image.canonical_png().unwrap()).unwrap();
    assert_eq!(reopened.dimensions(), (100, 100));
    assert_eq!(reopened.fingerprint(), image.fingerprint());
    assert_eq!(reopened.pixels, pixels);
}

#[test]
fn hide_target_covers_only_target_and_reveal_recovers_it() {
    let pixels = source();
    let hidden = render(&pixels, "hide_target", RenderPhase::Hidden);
    assert_eq!(*hidden.get_pixel(20, 20), MASK_PIXEL);
    assert_eq!(hidden.get_pixel(70, 70), pixels.get_pixel(70, 70));
    let revealed = render(&pixels, "hide_target", RenderPhase::Revealed);
    assert_eq!(revealed, pixels);
}

#[test]
fn hide_all_reveals_target_but_preserves_unrelated_occlusions() {
    let pixels = source();
    let hidden = render(&pixels, "hide_all", RenderPhase::Hidden);
    assert_eq!(*hidden.get_pixel(20, 20), MASK_PIXEL);
    assert_eq!(*hidden.get_pixel(70, 70), MASK_PIXEL);
    let revealed = render(&pixels, "hide_all", RenderPhase::Revealed);
    assert_eq!(revealed.get_pixel(20, 20), pixels.get_pixel(20, 20));
    assert_eq!(*revealed.get_pixel(70, 70), MASK_PIXEL);
}

#[test]
fn a_mask_replaces_all_rgba_channels_instead_of_blending() {
    let pixels = RgbaImage::from_pixel(100, 100, Rgba([201, 17, 90, 0]));
    let hidden = render(&pixels, "hide_target", RenderPhase::Hidden);
    assert_eq!(*hidden.get_pixel(20, 20), MASK_PIXEL);
    assert_eq!(*hidden.get_pixel(50, 50), Rgba([201, 17, 90, 0]));
}

#[test]
fn hidden_pixels_cannot_affect_a_downsampled_derivative() {
    let a = source();
    let mut b = a.clone();
    for y in 10..30 {
        for x in 10..30 {
            b.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }
    let q = question("hide_target");
    let a = image(&a)
        .render(&q, &id(4), RenderPhase::Hidden, 19)
        .unwrap();
    let b = image(&b)
        .render(&q, &id(4), RenderPhase::Hidden, 19)
        .unwrap();
    assert_eq!(a.png, b.png);
    assert_eq!((a.width, a.height), (19, 19));
}

#[test]
fn unrelated_overlapping_mask_keeps_its_pixels_covered_after_reveal() {
    let mut definition = serde_json::to_value(question("hide_all").definition()).unwrap();
    definition["question"]["masks"][1]["rect"] = json!({"x":0.2,"y":0.2,"width":0.2,"height":0.2});
    let q = ValidatedQuestion::parse(&definition.to_string()).unwrap();
    let output = image(&source())
        .render(&q, &id(4), RenderPhase::Revealed, 100)
        .unwrap();
    let pixels = CanonicalImage::decode(&output.png).unwrap().pixels;
    assert_eq!(*pixels.get_pixel(15, 15), Rgba([231, 117, 43, 255]));
    assert_eq!(*pixels.get_pixel(25, 25), MASK_PIXEL);
}

#[test]
fn multi_region_question_reveals_all_its_targets() {
    let mut definition = serde_json::to_value(question("hide_all").definition()).unwrap();
    definition["question"]["questions"][0]["maskIds"] = json!([id(2), id(3)]);
    let q = ValidatedQuestion::parse(&definition.to_string()).unwrap();
    let source = source();
    let image = image(&source);
    let output = image
        .render(&q, &id(4), RenderPhase::Revealed, 100)
        .unwrap();
    assert_eq!(output.target_rectangles.len(), 2);
    assert_eq!(CanonicalImage::decode(&output.png).unwrap().pixels, source);
}

#[test]
fn invalid_image_and_target_fail_instead_of_returning_an_original_image() {
    assert!(CanonicalImage::decode(b"not an image").is_err());
    assert!(CanonicalImage::decode(b"<svg><text>answer</text></svg>").is_err());
    assert!(CanonicalImage::decode(&[]).is_err());
    let image = image(&source());
    let q = question("hide_target");
    assert!(image.render(&q, &id(99), RenderPhase::Hidden, 100).is_err());
    assert!(image.render(&q, &id(4), RenderPhase::Hidden, 0).is_err());
    assert!(image.render(&q, &id(4), RenderPhase::Hidden, 4097).is_err());
}

#[test]
fn an_oversized_header_is_rejected_even_when_compressed_bytes_are_small() {
    let pixels = RgbaImage::from_pixel(MAX_IMAGE_EDGE + 1, 1, Rgba([0, 0, 0, 255]));
    let bytes = encode_png(&pixels).unwrap();
    assert!(bytes.len() < 100_000);
    assert!(CanonicalImage::decode(&bytes).is_err());
}

#[test]
fn jpeg_and_webp_use_the_same_shared_native_normalization_path() {
    let pixels = source();
    let rgb = DynamicImage::ImageRgba8(pixels.clone()).into_rgb8();
    let mut jpeg = Vec::new();
    JpegEncoder::new(&mut jpeg)
        .write_image(rgb.as_raw(), 100, 100, ExtendedColorType::Rgb8)
        .unwrap();
    let mut webp = Vec::new();
    WebPEncoder::new_lossless(&mut webp)
        .write_image(pixels.as_raw(), 100, 100, ExtendedColorType::Rgba8)
        .unwrap();
    assert_eq!(
        CanonicalImage::decode(&jpeg).unwrap().dimensions(),
        (100, 100)
    );
    let webp = CanonicalImage::decode(&webp).unwrap();
    assert_eq!(webp.pixels, pixels);
    assert!(webp
        .canonical_png()
        .unwrap()
        .starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[test]
fn exif_orientation_is_applied_once_and_metadata_is_not_copied() {
    let rgb = image::RgbImage::from_pixel(12, 7, image::Rgb([200, 17, 31]));
    let mut jpeg = Vec::new();
    JpegEncoder::new(&mut jpeg)
        .write_image(rgb.as_raw(), 12, 7, ExtendedColorType::Rgb8)
        .unwrap();
    // JPEG APP1: Exif + little-endian TIFF, one SHORT orientation tag (6).
    let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
    let mut oriented = jpeg[..2].to_vec();
    oriented.extend_from_slice(b"\xff\xe1");
    oriented.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
    oriented.extend_from_slice(exif);
    oriented.extend_from_slice(&jpeg[2..]);
    let image = CanonicalImage::decode(&oriented).unwrap();
    assert_eq!(image.dimensions(), (7, 12));
    let png = image.canonical_png().unwrap();
    assert!(!png.windows(4).any(|bytes| bytes == b"eXIf"));
    assert!(!png.windows(4).any(|bytes| bytes == b"tEXt"));
    assert_eq!(CanonicalImage::decode(&png).unwrap().dimensions(), (7, 12));
}

#[test]
fn cache_identity_tracks_source_geometry_phase_and_resolution_not_question_prose() {
    let image = image(&source());
    let q = question("hide_all");
    let initial = image.render(&q, &id(4), RenderPhase::Hidden, 100).unwrap();
    let revealed = image
        .render(&q, &id(4), RenderPhase::Revealed, 100)
        .unwrap();
    let smaller = image.render(&q, &id(4), RenderPhase::Hidden, 50).unwrap();
    assert_ne!(initial.cache_key, revealed.cache_key);
    assert_ne!(initial.cache_key, smaller.cache_key);
    let mut definition = serde_json::to_value(q.definition()).unwrap();
    definition["question"]["questions"][0]["answer"] = rich("Another private answer");
    let same = ValidatedQuestion::parse(&definition.to_string()).unwrap();
    let same = image
        .render(&same, &id(4), RenderPhase::Hidden, 100)
        .unwrap();
    assert_eq!(initial.cache_key, same.cache_key);
    definition["question"]["masks"][0]["rect"]["x"] = json!(0.11);
    let moved = ValidatedQuestion::parse(&definition.to_string()).unwrap();
    let moved = image
        .render(&moved, &id(4), RenderPhase::Hidden, 100)
        .unwrap();
    assert_ne!(initial.cache_key, moved.cache_key);
}

#[test]
fn bounded_png_writer_refuses_growth_past_the_limit() {
    let mut output = BoundedOutput(vec![0; MAX_IMAGE_BYTES - 1]);
    assert!(output.write_all(&[1, 2]).is_err());
    assert_eq!(output.0.len(), MAX_IMAGE_BYTES - 1);
    output.write_all(&[1]).unwrap();
    assert_eq!(output.0.len(), MAX_IMAGE_BYTES);
}

#[test]
fn tiny_images_remain_native_size_instead_of_being_upscaled() {
    let pixels = RgbaImage::from_pixel(1, 1, Rgba([1, 2, 3, 4]));
    let image = image(&pixels);
    let output = image
        .render(&question("hide_target"), &id(4), RenderPhase::Hidden, 4096)
        .unwrap();
    assert_eq!((output.width, output.height), (1, 1));
    assert_eq!(
        *CanonicalImage::decode(&output.png)
            .unwrap()
            .pixels
            .get_pixel(0, 0),
        MASK_PIXEL
    );
}
