use aihmc_api::texture::image::{avatar, decode_png, model_type};

#[test]
fn matches_ten_independent_gd_reference_images_pixel_for_pixel() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/testdata");
    let cases = [
        ("32", 8),
        ("32", 16),
        ("32", 20),
        ("32", 64),
        ("64", 8),
        ("64", 16),
        ("64", 20),
        ("64", 64),
        ("slim", 20),
        ("nohat", 20),
    ];
    for (name, size) in cases {
        let skin =
            decode_png(&std::fs::read(root.join(format!("skin-{name}.png"))).unwrap()).unwrap();
        let actual = image::load_from_memory(&avatar(&skin, size).unwrap())
            .unwrap()
            .into_rgba8();
        let reference = image::open(root.join(format!("avatar-{name}-{size}.png")))
            .unwrap()
            .into_rgba8();
        assert_eq!(actual, reference, "GD pixels differ for {name}/{size}");
    }
}

#[test]
fn preserves_reference_models_and_rejects_invalid_input() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let models: std::collections::HashMap<String, String> =
        serde_json::from_slice(&std::fs::read(root.join("docs/legacy-image-models.json")).unwrap())
            .unwrap();
    for (file, expected) in models {
        let skin =
            decode_png(&std::fs::read(root.join("tests/testdata").join(file)).unwrap()).unwrap();
        assert_eq!(model_type(&skin), expected);
        assert!(avatar(&skin, 0).is_err());
    }
    assert!(decode_png(b"not PNG").is_err());
}

#[test]
fn model_detection_uses_palette_indices_even_for_identical_colors() {
    let mut encoded = vec![];
    {
        let mut encoder = png::Encoder::new(&mut encoded, 64, 64);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_palette(vec![20, 30, 40, 20, 30, 40]);
        let mut writer = encoder.write_header().unwrap();
        let mut indices = vec![0; 64 * 64];
        indices[20 * 64 + 54] = 1;
        writer.write_image_data(&indices).unwrap();
    }
    assert_eq!(model_type(&decode_png(&encoded).unwrap()), "default");
}
