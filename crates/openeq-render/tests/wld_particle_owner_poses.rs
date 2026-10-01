//! Explicit placed-owner extraction into the diagnostic sampler; no scene effects.
use openeq_assets::loader;
use openeq_render::wld_particles::{
    DiagnosticRandom, FrameInput, OwnerPose, PokDefinition, Sampler,
};
use std::collections::BTreeMap;

#[test]
#[ignore = "requires the original Plane of Knowledge assets; CPU only"]
fn original_pok_particle_owner_poses_preserve_native_distortion_and_birth_origins() {
    let scene = loader::load_zone(loader::default_client_dir().unwrap(), "poknowledge").unwrap();
    let mut placements = 0;
    let mut extracted = 0;
    let mut admitted = 0;
    let mut rejected = BTreeMap::new();
    let mut witnessed = 0;
    for (instance_index, instance) in scene.instances.iter().enumerate() {
        let Some(source) = scene.wld_object_sources.get(&instance.object) else {
            continue;
        };
        if source.particle_attachments.is_empty() {
            continue;
        }
        placements += 1;
        let transforms = source
            .diagnostic_particle_owner_transforms(instance)
            .unwrap();
        assert_eq!(transforms.len(), source.particle_attachments.len());
        extracted += transforms.len();
        for transform in transforms {
            let attachment = &source.particle_attachments[transform.attachment_index];
            assert_eq!(transform.owner_track, attachment.owner_track);
            assert_eq!(transform.source_reference, attachment.source_reference);
            assert_eq!(
                transform.definition_reference,
                attachment.definition_reference
            );
            let rows = transform.native_world_rows;
            let origin = [rows[3][0], rows[3][1], rows[3][2]];
            // Captured original node matrices and first particle origins, not
            // normalized output from OpenEQ's mesh first-pose helper.
            let expected = match (instance_index, transform.definition_reference.0) {
                (17, 411) => Some(("ftorch301", [140.55469, 1375.0557, -100.84273])),
                (17, 406) => Some(("ftorch301", [140.55469, 1375.0557, -100.43257])),
                (570, 20) => Some(("poklamp500", [925.6063, 144.00928, -124.463806])),
                (253, 20) => Some(("poktorch500", [-783.1562, 666.51373, -143.42377])),
                (535, 438) => Some(("ftorch302", [-332.42532, 755.8621, -87.62789])),
                _ => None,
            };
            if let Some((name, expected_origin)) = expected {
                assert_eq!(instance.object, name);
                assert_eq!(origin, expected_origin);
                witnessed += 1;
            }
            let owner = match OwnerPose::from_native_world(rows) {
                Ok(owner) => owner,
                Err(_) => {
                    // This real packed quaternion is slightly non-unit. The
                    // extraction succeeds but the uniform-axis sampler rejects
                    // it; normalizing the source would incorrectly admit it.
                    assert_eq!(instance.object, "ftorch302");
                    *rejected.entry(instance.object.clone()).or_insert(0) += 1;
                    continue;
                }
            };
            assert_ne!(instance.object, "ftorch302");
            admitted += 1;
            assert_eq!(owner.origin(), origin);
            let mut sampler = Sampler::new(
                PokDefinition::from_cloud(&attachment.definition).unwrap(),
                -1,
            );
            sampler
                .update(
                    FrameInput {
                        delta_seconds: 0.,
                        owner,
                        camera_position: instance.position,
                        owner_suppressed: false,
                        radial_motion_visible: true,
                        draw_context: -1,
                        owner_alpha: 1.,
                    },
                    &mut DiagnosticRandom::seed_one(),
                    |_| true,
                )
                .unwrap();
            assert_eq!(sampler.particles().len(), 1);
            assert_eq!(sampler.particles()[0].birth_origin, origin);
        }
    }
    assert_eq!(
        (placements, extracted, admitted, witnessed),
        (366, 475, 462, 5)
    );
    assert_eq!(rejected, BTreeMap::from([("ftorch302".to_string(), 13)]));
    eprintln!(
        "{placements} placements, {extracted} raw owner matrices, {admitted} sampler admissions, 13 explicitly unsupported distorted FTORCH302 matrices; all five native origins exact"
    );
}
