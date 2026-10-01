//! Dumps the fragment table of a `WLD` stored inside an archive.
//!
//! ```text
//! wldscan /path/to/gfaydark.s3d            # every .wld in the archive
//! wldscan /path/to/gfaydark.s3d gfaydark.wld
//! ```

use openeq_assets::pfs::Archive;
use openeq_assets::wld::{Fragment, Wld};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: wldscan <archive> [file.wld]");
        std::process::exit(2);
    };
    let only = args.next().map(|name| name.to_ascii_lowercase());

    let archive = Archive::open(&path)?;
    let wld_names: Vec<String> = archive
        .names()
        .iter()
        .filter(|name| name.to_ascii_lowercase().ends_with(".wld"))
        .filter(|name| {
            only.as_ref()
                .is_none_or(|only| name.to_ascii_lowercase() == *only)
        })
        .cloned()
        .collect();

    for name in wld_names {
        let wld = Wld::open(&archive, &name)?;
        println!(
            "== {name} ({} fragments, new_format={})",
            wld.chunks().len(),
            wld.new_format
        );
        for (index, chunk) in wld.chunks().iter().enumerate() {
            println!(
                "  [{index:4}] 0x{:02X} {:?} {}",
                chunk.fragment.type_code(),
                chunk.name,
                describe(&chunk.fragment)
            );
        }
    }

    Ok(())
}

fn describe(fragment: &Fragment) -> String {
    use Fragment::*;
    match fragment {
        TextureList(list) => format!("{} names", list.filenames.len()),
        Animation(animation) => format!(
            "frame_time={} refs={}",
            animation.frame_time,
            animation.textures.len()
        ),
        AnimationRef(reference) => format!("-> {:?}", reference.animation),
        Skeleton(skeleton) => format!(
            "{} tracks, {} meshes",
            skeleton.tracks.len(),
            skeleton.meshes.len()
        ),
        SkeletonRef(reference) => format!("-> {:?}", reference.skeleton),
        PieceTrack(track) => format!("{} frames", track.frames.len()),
        PieceTrackRef(reference) => format!("-> {:?} speed={:?}", reference.track, reference.speed),
        ActorDef(def) => format!("{} refs", def.references.len()),
        ActorInstance(instance) => format!("-> {:?} at {:?}", instance.actor, instance.position),
        LightSource(source) => format!("color={:?}", source.color),
        LightSourceRef(reference) => format!("-> {:?}", reference.source),
        Light(light) => format!("radius={} at {:?}", light.radius, light.position),
        MeshRef(reference) => format!("-> {:?}", reference.mesh),
        Material(material) => format!("flags=0x{:X} anim={:?}", material.flags, material.animation),
        MaterialList(list) => format!("{} materials", list.materials.len()),
        ParticleCloud(cloud) => format!(
            "flags=0x{:X} texture={:?} tail={} bytes; playback unsupported",
            cloud.flags(),
            cloud.texture_reference,
            cloud.tail.len()
        ),
        Mesh(mesh) => format!(
            "{} verts, {} polys, {} matrefs, {} polytex",
            mesh.vertices.len(),
            mesh.polygons.len(),
            mesh.polygon_textures.len(),
            mesh.polygon_textures.len()
        ),
        Ignored(code) => format!("ignored type 0x{code:02X}"),
    }
}
