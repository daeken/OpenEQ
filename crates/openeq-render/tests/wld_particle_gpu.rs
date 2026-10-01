//! A WebGPU diagnostic contract, not comparison against original-client pixels.
use openeq_assets::texture::Texture;
use openeq_render::wld_particle_gpu::*;

fn texture(width: u32, height: u32, rgba: Vec<u8>) -> Texture {
    Texture {
        name: "diagnostic".into(),
        width,
        height,
        rgba,
    }
}

fn quad(rect: [f32; 4], depth: f32, diffuse: u32, texture: usize) -> ProjectedQuad {
    let [l, t, r, b] = rect;
    ProjectedQuad {
        vertices: [
            ([l, t], [0., 1.]),
            ([r, t], [1., 1.]),
            ([r, b], [1., 0.]),
            ([l, b], [0., 0.]),
        ]
        .map(|(xy, uv)| ProjectedVertex {
            xyzrhw: [xy[0], xy[1], depth, 1.],
            diffuse,
            specular: 0,
            uv,
        }),
        texture,
    }
}

fn frame<'a>(
    textures: &'a [Texture],
    quads: &'a [ProjectedQuad],
    size: [u32; 2],
) -> DiagnosticFrame<'a> {
    DiagnosticFrame {
        target_size: size,
        viewport: Viewport {
            x: 0,
            y: 0,
            width: size[0],
            height: size[1],
        },
        pixel_centers: PixelCenterConvention::AsCaptured,
        clear_rgba: [0.; 4],
        clear_depth: 1.,
        textures,
        quads,
    }
}

fn pixel(image: &DiagnosticImage, x: u32, y: u32) -> [u8; 4] {
    let p = ((y * image.width + x) * 4) as usize;
    image.rgba[p..p + 4].try_into().unwrap()
}

fn close(actual: [u8; 4], expected: [u8; 4], tolerance: u8) {
    assert!(
        actual
            .into_iter()
            .zip(expected)
            .all(|(a, b)| a.abs_diff(b) <= tolerance),
        "actual {actual:?}, expected {expected:?}"
    );
}

#[test]
fn invalid_frames_are_rejected_before_gpu_work() {
    let textures = [texture(1, 1, vec![255; 4])];
    let good = quad([0., 0., 4., 4.], 0.5, u32::MAX, 0);
    let quads = [good];
    let mut input = frame(&textures, &quads, [4, 4]);
    assert!(input.validate().is_ok());
    for size in [
        [0, 4],
        [4, 0],
        [MAX_TARGET_EDGE + 1, 4],
        [u32::MAX, u32::MAX],
    ] {
        input.target_size = size;
        assert!(input.validate().is_err());
    }
    input.target_size = [4, 4];
    input.viewport.x = u32::MAX;
    assert!(input.validate().is_err());
    input.viewport.x = 0;
    input.viewport.width = 0;
    assert!(input.validate().is_err());
    input.viewport.width = 4;
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        input.clear_depth = bad;
        assert!(input.validate().is_err());
        for field in 0..6 {
            let mut changed = good;
            if field < 4 {
                changed.vertices[0].xyzrhw[field] = bad;
            } else {
                changed.vertices[0].uv[field - 4] = bad;
            }
            assert!(frame(&textures, &[changed], [4, 4]).validate().is_err());
        }
    }
    input.clear_depth = 1.;
    input.clear_rgba[0] = 1.1;
    assert!(input.validate().is_err());
    for rhw in [0., -1., f32::from_bits(1), f32::MIN_POSITIVE / 2., f32::MAX] {
        let mut changed = good;
        changed.vertices[0].xyzrhw[3] = rhw;
        assert!(frame(&textures, &[changed], [4, 4]).validate().is_err());
    }
    let mut changed = good;
    changed.vertices[0].xyzrhw[0] = f32::MAX;
    changed.vertices[0].xyzrhw[3] = 0.1;
    assert!(frame(&textures, &[changed], [4, 4]).validate().is_err());
    changed = good;
    changed.vertices[0].specular = 1;
    assert!(frame(&textures, &[changed], [4, 4]).validate().is_err());
    changed = good;
    changed.texture = 1;
    assert!(frame(&textures, &[changed], [4, 4]).validate().is_err());
    assert!(
        frame(&textures, &vec![good; MAX_QUADS + 1], [4, 4])
            .validate()
            .is_err()
    );
    assert!(
        frame(&vec![textures[0].clone(); MAX_TEXTURES + 1], &[], [4, 4])
            .validate()
            .is_err()
    );
    for source in [
        texture(0, 1, vec![]),
        texture(1, 1, vec![0; 3]),
        texture(MAX_TEXTURE_EDGE + 1, 1, vec![]),
        texture(u32::MAX, u32::MAX, vec![]),
    ] {
        assert!(frame(&[source], &[], [4, 4]).validate().is_err());
    }
    let budget = [
        texture(
            MAX_TEXTURE_EDGE,
            MAX_TEXTURE_EDGE,
            vec![0; MAX_TEXTURE_BYTES as usize],
        ),
        textures[0].clone(),
    ];
    assert!(frame(&budget, &[], [4, 4]).validate().is_err());
    // Retain finite out-of-range depth for GPU clipping; do not invent a clamp.
    changed = good;
    changed.vertices[0].xyzrhw[2] = -0.5;
    assert!(frame(&textures, &[changed], [4, 4]).validate().is_ok());
}

#[test]
#[ignore = "requires headless GPU"]
fn byte_space_modulation_and_srcalpha_add_include_native_alpha_equation() {
    let gpu = DiagnosticRenderer::new_headless().unwrap();
    let textures = [texture(1, 1, vec![128, 64, 192, 128])];
    let quads = [quad([0., 0., 8., 8.], 0.5, 0x8080c040, 0)];
    let mut input = frame(&textures, &quads, [8, 8]);
    input.clear_rgba = [10., 20., 30., 40.].map(|v| v / 255.);
    let first = gpu.render(&input).unwrap();
    // Independent byte arithmetic: source RGB * diffuse RGB * source A *
    // diffuse A, added to background; output alpha adds (source A*diffuse A)^2.
    let a = (128. / 255.) * (128. / 255.);
    let expected = [
        10. + 128. * 128. / 255. * a,
        20. + 64. * 192. / 255. * a,
        30. + 192. * 64. / 255. * a,
        40. + 255. * a * a,
    ]
    .map(|v: f64| v.round() as u8);
    close(pixel(&first, 4, 4), expected, 1);
    let twice = [quads[0], quads[0]];
    input.quads = &twice;
    let second = gpu.render(&input).unwrap();
    let doubled = [
        10. + 2. * 128. * 128. / 255. * a,
        20. + 2. * 64. * 192. / 255. * a,
        30. + 2. * 192. * 64. / 255. * a,
        40. + 2. * 255. * a * a,
    ]
    .map(|v: f64| v.round() as u8);
    close(pixel(&second, 4, 4), doubled, 2);
    assert_eq!(first.uploaded_texture_sizes, [[1, 1]]);
}

#[test]
#[ignore = "requires headless GPU"]
fn nominal_alpha_cutoff_includes_one_over_255_and_rejects_lower_products() {
    let gpu = DiagnosticRenderer::new_headless().unwrap();
    let textures = [texture(1, 1, vec![255, 255, 255, 1])];
    for (alpha, expected) in [(0, 0), (128, 0), (254, 0), (255, 1)] {
        let quads = [quad([0., 0., 4., 4.], 0.5, (alpha << 24) | 0xffffff, 0)];
        let image = gpu.render(&frame(&textures, &quads, [4, 4])).unwrap();
        assert_eq!(pixel(&image, 2, 2), [expected, expected, expected, 0]);
    }
}

#[test]
#[ignore = "requires headless GPU"]
fn depth_equality_passes_without_writes_and_outside_depth_is_clipped() {
    let gpu = DiagnosticRenderer::new_headless().unwrap();
    let textures = [texture(1, 1, vec![255; 4])];
    let quads = [
        quad([0., 0., 4., 4.], 0.5, 0xff400000, 0),
        quad([0., 0., 4., 4.], 0.25, 0xff004000, 0),
        quad([0., 0., 4., 4.], 0.5, 0xff000040, 0),
        quad([0., 0., 4., 4.], 0.75, 0xffffffff, 0),
    ];
    let mut input = frame(&textures, &quads, [4, 4]);
    input.clear_depth = 0.5;
    assert_eq!(pixel(&gpu.render(&input).unwrap(), 2, 2), [64, 64, 64, 255]);
    for depth in [-0.1, 1.1] {
        let outside = [quad([0., 0., 4., 4.], depth, u32::MAX, 0)];
        // Clear to 1 so incorrectly clamping either depth to [0,1] would draw.
        let clipped = frame(&textures, &outside, [4, 4]);
        assert_eq!(pixel(&gpu.render(&clipped).unwrap(), 2, 2), [0; 4]);
    }
}

#[test]
#[ignore = "requires headless GPU"]
fn original_size_texels_keep_captured_v_orientation_and_wrap_linear_sampling() {
    let gpu = DiagnosticRenderer::new_headless().unwrap();
    let rgba: Vec<_> = [
        [17, 31, 47, 255],
        [61, 79, 97, 255],
        [113, 127, 139, 255],
        [151, 163, 179, 255],
        [191, 203, 211, 255],
        [223, 239, 251, 255],
    ]
    .into_iter()
    .flatten()
    .collect();
    let textures = [texture(3, 2, rgba.clone())];
    let quads = [quad([0., 0., 3., 2.], 0.5, u32::MAX, 0)];
    let image = gpu.render(&frame(&textures, &quads, [3, 2])).unwrap();
    assert_eq!(image.uploaded_texture_sizes, [[3, 2]]);
    let expected: Vec<_> = rgba[12..]
        .iter()
        .chain(rgba[..12].iter())
        .copied()
        .collect();
    assert_eq!(
        image.rgba, expected,
        "captured top V=1 must sample the bottom texture row"
    );

    let textures = [texture(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255])];
    for (u, expected) in [
        (0., [128, 0, 128, 255]),
        (1.25, [255, 0, 0, 255]),
        (-0.25, [0, 0, 255, 255]),
    ] {
        let mut q = quad([0., 0., 4., 4.], 0.5, u32::MAX, 0);
        for v in &mut q.vertices {
            v.uv = [u, 0.5];
        }
        let image = gpu.render(&frame(&textures, &[q], [4, 4])).unwrap();
        close(pixel(&image, 2, 2), expected, 1);
    }
}

#[test]
#[ignore = "requires headless GPU"]
fn native_diagonal_interpolates_between_top_right_and_bottom_left() {
    let gpu = DiagnosticRenderer::new_headless().unwrap();
    let textures = [texture(1, 1, vec![255; 4])];
    let mut q = quad([0., 0., 8., 8.], 0.5, 0xff000000, 0);
    q.vertices[1].diffuse = 0xffff0000;
    q.vertices[3].diffuse = 0xff0000ff;
    let image = gpu.render(&frame(&textures, &[q], [8, 8])).unwrap();
    // Pixel center (3.5,3.5) has barycentric weights (1/8,7/16,7/16)
    // in native triangle TL,TR,BL. The alternate TL-to-BR diagonal would
    // interpolate only the two black corners here. This is a topology test,
    // not evidence for native interpolation of varying diffuse values.
    close(pixel(&image, 3, 3), [112, 0, 112, 255], 1);
}

#[test]
#[ignore = "requires headless GPU"]
fn viewport_and_half_pixel_choice_are_explicit_without_changing_rhw_projection() {
    let gpu = DiagnosticRenderer::new_headless().unwrap();
    let textures = [texture(1, 1, vec![255; 4])];
    let mut q = quad([2.25, 2.25, 3.25, 3.25], 0.5, u32::MAX, 0);
    for v in &mut q.vertices {
        v.xyzrhw[3] = 1. / 30.;
    }
    let quads = [q];
    let mut input = frame(&textures, &quads, [8, 8]);
    input.viewport = Viewport {
        x: 2,
        y: 2,
        width: 4,
        height: 4,
    };
    let unshifted = gpu.render(&input).unwrap();
    assert_eq!(pixel(&unshifted, 2, 2), [255; 4]);
    assert_eq!(pixel(&unshifted, 3, 3), [0; 4]);
    input.pixel_centers = PixelCenterConvention::ShiftByHalfPixel;
    let shifted = gpu.render(&input).unwrap();
    assert_eq!(pixel(&shifted, 2, 2), [0; 4]);
    assert_eq!(pixel(&shifted, 3, 3), [255; 4]);
    assert_eq!(pixel(&shifted, 0, 0), [0; 4]);
    input.quads = &[];
    assert!(gpu.render(&input).unwrap().rgba.iter().all(|b| *b == 0));
}

#[test]
#[ignore = "requires original PoK textures and headless GPU"]
fn original_textures_render_the_four_captured_native_quads_without_resizing() {
    let base = openeq_assets::loader::default_client_dir().unwrap();
    let archive = openeq_assets::pfs::Archive::open(base.join("poknowledge_obj.s3d")).unwrap();
    let textures: Vec<_> = ["csmoke.dds", "geng00.dds"]
        .map(|name| Texture::decode(name, &archive.read(name).unwrap()).unwrap())
        .into();
    assert_eq!((textures[0].width, textures[0].height), (16, 32));
    assert_eq!((textures[1].width, textures[1].height), (64, 64));
    let gpu = DiagnosticRenderer::new_headless().unwrap();
    // Original converter/update outputs from the GPU-contract probe. These are
    // captured screen coordinates, not a second implementation of its camera.
    let cases = [
        (
            "csmoke",
            [420., 253.333_33, 433.333_3, 266.666_66],
            0xff646464,
            0,
        ),
        (
            "l301",
            [420., 253.333_33, 433.333_3, 266.666_66],
            u32::MAX,
            1,
        ),
        (
            "l308",
            [425., 258.333_34, 428.333_3, 261.666_66],
            u32::MAX,
            1,
        ),
        ("l500", [418.666_66, 252., 434.666_66, 268.], u32::MAX, 1),
    ];
    for (name, rect, diffuse, texture) in cases {
        let mut q = quad(rect, 0.966_666_64, diffuse, texture);
        for v in &mut q.vertices {
            v.xyzrhw[3] = 0.033_333_335;
        }
        let quads = [q];
        let image = gpu.render(&frame(&textures, &quads, [800, 600])).unwrap();
        assert_eq!(image.uploaded_texture_sizes, [[16, 32], [64, 64]]);
        assert!(
            image
                .rgba
                .chunks_exact(4)
                .any(|p| p[..3].iter().any(|c| *c > 0)),
            "{name}"
        );
        assert!(
            pixel(&image, 426, 260)[..3].iter().any(|c| *c > 0),
            "{name}"
        );
        image::save_buffer(
            format!("/tmp/openeq-wld-particle-gpu-{name}.png"),
            &image.rgba,
            image.width,
            image.height,
            image::ColorType::Rgba8,
        )
        .unwrap();
    }
}
