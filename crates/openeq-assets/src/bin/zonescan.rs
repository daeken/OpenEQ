//! Command line inspector for EverQuest zone assets.
//!
//! ```text
//! zonescan akanon
//! zonescan gfaydark --obj /tmp/gfaydark.obj --textures /tmp/gfaydark-tex
//! ```
//!
//! Loading a zone exercises the whole asset stack (archive, fragments, meshes,
//! materials); exporting an OBJ and PNGs makes the result easy to eyeball
//! outside the engine.

use std::io::Write;
use std::path::PathBuf;

use openeq_assets::mesh::VERTEX_STRIDE;
use openeq_assets::{Scene, loader};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(zone) = args.next() else {
        eprintln!("usage: zonescan <zone> [--dir DIR] [--obj FILE] [--textures DIR]");
        std::process::exit(2);
    };

    let mut dir = loader::default_client_dir();
    let mut obj_out = None;
    let mut texture_dir = None;
    let mut preview_out = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dir" => dir = args.next().map(PathBuf::from),
            "--obj" => obj_out = args.next().map(PathBuf::from),
            "--textures" => texture_dir = args.next().map(PathBuf::from),
            "--preview" => preview_out = args.next().map(PathBuf::from),
            other => {
                anyhow::bail!("unrecognised argument {other}");
            }
        }
    }

    let dir = dir.ok_or_else(|| anyhow::anyhow!("no client directory (use --dir)"))?;
    let scene = loader::load_zone(&dir, &zone)?;

    report(&scene);

    if let Some(path) = obj_out {
        write_obj(&scene, &path)?;
        println!("wrote {}", path.display());
    }
    if let Some(path) = texture_dir {
        write_textures(&scene, &path)?;
        println!("wrote textures to {}", path.display());
    }
    if let Some(path) = preview_out {
        preview(&scene, &path)?;
        println!("wrote preview to {}", path.display());
    }

    Ok(())
}

/// Renders a shaded top-down view with a depth buffer.
///
/// This is a debugging aid: it confirms that positions, winding and scale
/// survived the asset pipeline without needing a GPU.
fn preview(scene: &Scene, path: &std::path::Path) -> anyhow::Result<()> {
    const SIZE: u32 = 1200;
    let mut color = vec![20u8; (SIZE * SIZE * 3) as usize];
    let mut depth = vec![f32::INFINITY; (SIZE * SIZE) as usize];

    let (mut min, mut max) = ([f32::MAX; 2], [f32::MIN; 2]);
    for geometry in &scene.meshes {
        for vertex in geometry.vertices.chunks_exact(VERTEX_STRIDE) {
            for axis in 0..2 {
                min[axis] = min[axis].min(vertex[axis]);
                max[axis] = max[axis].max(vertex[axis]);
            }
        }
    }
    let span = (max[0] - min[0]).max(max[1] - min[1]).max(1.0);
    let scale = SIZE as f32 / span;
    let project = |x: f32, y: f32| [(x - min[0]) * scale, SIZE as f32 - (y - min[1]) * scale];

    for geometry in &scene.meshes {
        for triangle in geometry.indices.chunks_exact(3) {
            let mut points = [[0f32; 3]; 3];
            for (slot, index) in triangle.iter().enumerate() {
                let base = (*index as usize) * VERTEX_STRIDE;
                let vertex = &geometry.vertices[base..base + VERTEX_STRIDE];
                points[slot] = [vertex[0], vertex[1], vertex[2]];
            }
            let normal = face_normal(points);
            let shade = (normal[2].abs() * 0.7 + normal[1].abs() * 0.3).clamp(0.05, 1.0);
            rasterize(
                &mut color,
                &mut depth,
                SIZE,
                [
                    project(points[0][0], points[0][1]),
                    project(points[1][0], points[1][1]),
                    project(points[2][0], points[2][1]),
                ],
                [points[0][2], points[1][2], points[2][2]],
                shade,
            );
        }
    }

    let mut image = image::RgbImage::new(SIZE, SIZE);
    for (index, pixel) in color.chunks_exact(3).enumerate() {
        image.put_pixel(
            (index as u32) % SIZE,
            (index as u32) / SIZE,
            image::Rgb([pixel[0], pixel[1], pixel[2]]),
        );
    }
    image.save(path)?;
    Ok(())
}

fn face_normal(points: [[f32; 3]; 3]) -> [f32; 3] {
    let a = [
        points[1][0] - points[0][0],
        points[1][1] - points[0][1],
        points[1][2] - points[0][2],
    ];
    let b = [
        points[2][0] - points[0][0],
        points[2][1] - points[0][1],
        points[2][2] - points[0][2],
    ];
    let n = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
    [n[0] / length, n[1] / length, n[2] / length]
}

fn rasterize(
    color: &mut [u8],
    depth: &mut [f32],
    size: u32,
    screen: [[f32; 2]; 3],
    world_z: [f32; 3],
    shade: f32,
) {
    let min_x = screen
        .iter()
        .map(|p| p[0])
        .fold(f32::MAX, f32::min)
        .max(0.0) as i32;
    let max_x = screen
        .iter()
        .map(|p| p[0])
        .fold(f32::MIN, f32::max)
        .min(size as f32 - 1.0) as i32;
    let min_y = screen
        .iter()
        .map(|p| p[1])
        .fold(f32::MAX, f32::min)
        .max(0.0) as i32;
    let max_y = screen
        .iter()
        .map(|p| p[1])
        .fold(f32::MIN, f32::max)
        .min(size as f32 - 1.0) as i32;

    let area = edge(screen[0], screen[1], screen[2]);
    if area.abs() < 1e-6 {
        return;
    }

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let point = [x as f32 + 0.5, y as f32 + 0.5];
            let w0 = edge(screen[1], screen[2], point) / area;
            let w1 = edge(screen[2], screen[0], point) / area;
            let w2 = edge(screen[0], screen[1], point) / area;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let z = w0 * world_z[0] + w1 * world_z[1] + w2 * world_z[2];
            let index = (y as u32 * size + x as u32) as usize;
            if z < depth[index] {
                depth[index] = z;
                let value = (shade * 235.0) as u8;
                color[index * 3] = value;
                color[index * 3 + 1] = value;
                color[index * 3 + 2] = value;
            }
        }
    }
}

fn edge(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn report(scene: &Scene) {
    println!("zone: {}", scene.name);
    println!(
        "  materials: {}, meshes: {}, triangles: {}",
        scene.materials.len(),
        scene.meshes.len(),
        scene.triangle_count()
    );
    println!(
        "  objects: {}, instances: {}, lights: {}",
        scene.objects.len(),
        scene.instances.len(),
        scene.lights.len()
    );

    let animated = scene
        .materials
        .iter()
        .filter(|material| material.textures.len() > 1)
        .count();
    let masked = scene
        .materials
        .iter()
        .filter(|material| material.alpha_mask)
        .count();
    let transparent = scene
        .materials
        .iter()
        .filter(|material| material.transparent)
        .count();
    println!("  animated: {animated}, alpha-masked: {masked}, transparent: {transparent}");

    let textures = scene.texture_names();
    println!("  textures referenced: {}", textures.len());
    for name in textures.iter().take(5) {
        println!("    {name}");
    }
    if textures.len() > 5 {
        println!("    ... and {} more", textures.len() - 5);
    }
}

fn write_obj(scene: &Scene, path: &std::path::Path) -> anyhow::Result<()> {
    let mut out = std::io::BufWriter::new(std::fs::File::create(path)?);
    writeln!(
        out,
        "# {} ({} triangles)",
        scene.name,
        scene.triangle_count()
    )?;

    for index in 0..scene.materials.len() {
        writeln!(out, "usemtl material{index}")?;
    }

    let mut vertex_offset = 1u32;
    let mut uv_offset = 1u32;
    for geometry in &scene.meshes {
        let vertex_count = geometry.vertex_count();
        for vertex in geometry.vertices.chunks_exact(VERTEX_STRIDE) {
            writeln!(out, "v {} {} {}", vertex[0], vertex[1], vertex[2])?;
        }
        for vertex in geometry.vertices.chunks_exact(VERTEX_STRIDE) {
            writeln!(out, "vt {} {}", vertex[6], 1.0 - vertex[7])?;
        }
        for vertex in geometry.vertices.chunks_exact(VERTEX_STRIDE) {
            writeln!(out, "vn {} {} {}", vertex[3], vertex[4], vertex[5])?;
        }
        writeln!(out, "usemtl material{}", geometry.material)?;
        for triangle in geometry.indices.chunks_exact(3) {
            let a = triangle[0] + vertex_offset;
            let b = triangle[1] + vertex_offset;
            let c = triangle[2] + vertex_offset;
            let uv_a = triangle[0] + uv_offset;
            let uv_b = triangle[1] + uv_offset;
            let uv_c = triangle[2] + uv_offset;
            writeln!(out, "f {a}/{uv_a}/{a} {b}/{uv_b}/{b} {c}/{uv_c}/{c}")?;
        }
        vertex_offset += vertex_count as u32;
        uv_offset += vertex_count as u32;
    }
    Ok(())
}

fn write_textures(scene: &Scene, dir: &std::path::Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut written = 0;
    for name in scene.texture_names() {
        let Some(texture) = scene.texture(&name) else {
            continue;
        };
        let file_name = format!(
            "{}.png",
            name.rsplit('/').next().unwrap_or(&name).replace('.', "_")
        );
        let buffer = image::RgbaImage::from_raw(texture.width, texture.height, texture.rgba)
            .ok_or_else(|| anyhow::anyhow!("texture {name} has inconsistent size"))?;
        buffer.save(dir.join(file_name))?;
        written += 1;
    }
    println!("decoded {written} textures");
    Ok(())
}
