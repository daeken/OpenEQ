//! Authored particle texture-chain metadata, without runtime cache emulation.

use crate::wld::{
    AnimationRef, Fragment, FragmentKind, ParticleTexture, Ref, SkeletonRef, TextureList, Wld,
};

// Native texture assembly has at most 16 atlas cells. Do not clone/expand
// larger records, and bound all variable-size strings and extension bytes.
const MAX_PARTICLE_TEXTURE_FRAMES: usize = 16;
const MAX_PARTICLE_TEXTURE_LAYERS: usize = 16;
const MAX_PARTICLE_TEXTURE_BYTES: usize = 4096;

/// A bounded inspection of the authored `0x26 -> 0x05 -> 0x04 -> 0x03` chain.
/// Empty `issues` validates the source family seen in the native PoK witness;
/// it does not authorize playback or establish the client's cache/blend state.
#[derive(Debug, Clone)]
pub struct ObjectParticleTexture {
    pub wld_filename: String,
    pub source_reference: Option<Ref>,
    pub binding: Option<ObjectParticleTextureNode<ParticleTexture>>,
    pub animation_reference: Option<ObjectParticleTextureNode<SkeletonRef>>,
    pub animation: Option<ObjectParticleTextureNode<AnimationRef>>,
    /// Source frame order, with unresolved entries retained as `None`. Empty
    /// when animation resolution fails or metadata limits prevent expansion.
    pub frames: Vec<Option<ObjectParticleTextureNode<TextureList>>>,
    pub issues: Vec<ObjectParticleTextureIssue>,
}

/// Exact source ownership of one texture-chain edge and its resolved target.
#[derive(Debug, Clone)]
pub struct ObjectParticleTextureNode<T> {
    pub source_reference: Ref,
    /// One-based source fragment identity, never collapsed by its name.
    pub definition_reference: Ref,
    pub name: String,
    pub definition: T,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectParticleTextureIssue {
    MissingTextureReference,
    MissingReference {
        source_reference: Ref,
    },
    Cycle {
        source_reference: Ref,
        definition_reference: Ref,
    },
    UnexpectedFragment {
        source_reference: Ref,
        definition_reference: Ref,
        expected_kind: u32,
        actual_kind: u32,
    },
    UnsupportedLayout {
        definition_reference: Ref,
        reason: &'static str,
    },
    UnsupportedMaterial {
        material: u32,
    },
    MetadataLimit {
        source_reference: Ref,
        definition_reference: Ref,
        field: &'static str,
        count: usize,
        limit: usize,
    },
    UnrepresentableIdentity {
        source_reference: Ref,
        fragment_index: usize,
    },
}

fn particle_texture_metadata_limit(fragment: &Fragment) -> Option<(&'static str, usize, usize)> {
    let check = |field, count, limit| (count > limit).then_some((field, count, limit));
    match fragment {
        Fragment::ParticleTexture(definition) => check(
            "binding tail bytes",
            definition.tail.len(),
            MAX_PARTICLE_TEXTURE_BYTES,
        ),
        Fragment::AnimationRef(definition) => check(
            "animation-reference tail bytes",
            definition.tail.len(),
            MAX_PARTICLE_TEXTURE_BYTES,
        ),
        Fragment::Animation(definition) => check(
            "frame references",
            definition.textures.len(),
            MAX_PARTICLE_TEXTURE_FRAMES,
        )
        .or_else(|| {
            check(
                "animation tail bytes",
                definition.tail.len(),
                MAX_PARTICLE_TEXTURE_BYTES,
            )
        }),
        Fragment::TextureList(definition) => check(
            "bitmap layers",
            definition.filenames.len(),
            MAX_PARTICLE_TEXTURE_LAYERS,
        )
        .or_else(|| {
            definition.filenames.iter().find_map(|filename| {
                check("filename bytes", filename.len(), MAX_PARTICLE_TEXTURE_BYTES)
            })
        })
        .or_else(|| {
            check(
                "bitmap tail bytes",
                definition.tail.len(),
                MAX_PARTICLE_TEXTURE_BYTES,
            )
        }),
        _ => None,
    }
}

fn particle_texture_node<T: FragmentKind + Clone>(
    wld: &Wld,
    source_reference: Ref,
    expected_kind: u32,
    ancestors: &mut Vec<usize>,
    issues: &mut Vec<ObjectParticleTextureIssue>,
) -> Option<ObjectParticleTextureNode<T>> {
    let Some(chunk) = wld.resolve(source_reference) else {
        issues.push(ObjectParticleTextureIssue::MissingReference { source_reference });
        return None;
    };
    let index = source_reference.fragment_index().unwrap_or_else(|| {
        wld.chunks()
            .iter()
            .position(|candidate| std::ptr::eq(candidate, chunk))
            .expect("resolved texture chunk belongs to its source")
    });
    // WLD's fragment count is encoded in 32 bits; reject an identity that
    // cannot be represented by a positive signed source reference.
    let Ok(identity) = i32::try_from(index + 1) else {
        issues.push(ObjectParticleTextureIssue::UnrepresentableIdentity {
            source_reference,
            fragment_index: index,
        });
        return None;
    };
    let definition_reference = Ref(identity);
    if ancestors.contains(&index) {
        issues.push(ObjectParticleTextureIssue::Cycle {
            source_reference,
            definition_reference,
        });
        return None;
    }
    let Some(definition) = T::of(&chunk.fragment) else {
        issues.push(ObjectParticleTextureIssue::UnexpectedFragment {
            source_reference,
            definition_reference,
            expected_kind,
            actual_kind: chunk.fragment.type_code(),
        });
        return None;
    };
    let limit = (chunk.name.len() > MAX_PARTICLE_TEXTURE_BYTES)
        .then_some((
            "fragment name bytes",
            chunk.name.len(),
            MAX_PARTICLE_TEXTURE_BYTES,
        ))
        .or_else(|| particle_texture_metadata_limit(&chunk.fragment));
    if let Some((field, count, limit)) = limit {
        issues.push(ObjectParticleTextureIssue::MetadataLimit {
            source_reference,
            definition_reference,
            field,
            count,
            limit,
        });
        return None;
    }
    ancestors.push(index);
    Some(ObjectParticleTextureNode {
        source_reference,
        definition_reference,
        name: chunk.name.clone(),
        definition: definition.clone(),
    })
}

pub(super) fn particle_texture_source(
    wld: &Wld,
    source_reference: Option<Ref>,
) -> ObjectParticleTexture {
    let mut source = ObjectParticleTexture {
        wld_filename: wld.filename.clone(),
        source_reference,
        binding: None,
        animation_reference: None,
        animation: None,
        frames: Vec::new(),
        issues: Vec::new(),
    };
    let Some(reference) = source_reference else {
        source
            .issues
            .push(ObjectParticleTextureIssue::MissingTextureReference);
        return source;
    };
    let mut ancestors = Vec::with_capacity(4);
    source.binding =
        particle_texture_node(wld, reference, 0x26, &mut ancestors, &mut source.issues);
    let Some(binding) = &source.binding else {
        return source;
    };
    if binding.definition.flags != 0 || !binding.definition.tail.is_empty() {
        source
            .issues
            .push(ObjectParticleTextureIssue::UnsupportedLayout {
                definition_reference: binding.definition_reference,
                reason: "unproven particle texture flags or trailing data",
            });
    }
    if binding.definition.material != 0x8000_0017 {
        source
            .issues
            .push(ObjectParticleTextureIssue::UnsupportedMaterial {
                material: binding.definition.material,
            });
    }
    source.animation_reference = particle_texture_node(
        wld,
        binding.definition.texture,
        0x05,
        &mut ancestors,
        &mut source.issues,
    );
    let Some(link) = &source.animation_reference else {
        return source;
    };
    if link.definition.flags != 0 || !link.definition.tail.is_empty() {
        source
            .issues
            .push(ObjectParticleTextureIssue::UnsupportedLayout {
                definition_reference: link.definition_reference,
                reason: "unproven particle animation-reference flags or trailing data",
            });
    }
    source.animation = particle_texture_node(
        wld,
        link.definition.animation,
        0x04,
        &mut ancestors,
        &mut source.issues,
    );
    let Some(animation) = &source.animation else {
        return source;
    };
    let definition = &animation.definition;
    if definition.flags != 0x18 || definition.parameter.is_some() || !definition.tail.is_empty() {
        source
            .issues
            .push(ObjectParticleTextureIssue::UnsupportedLayout {
                definition_reference: animation.definition_reference,
                reason: "unproven particle animation flags, parameter or trailing data",
            });
    }
    if definition.textures.len() != 1 || definition.frame_time != 100 {
        source
            .issues
            .push(ObjectParticleTextureIssue::UnsupportedLayout {
                definition_reference: animation.definition_reference,
                reason: "unproven particle frame count or interval",
            });
    }
    for &reference in &definition.textures {
        // Frame reuse is valid. Only ancestors on this edge form a cycle.
        let mut frame_ancestors = ancestors.clone();
        let bitmap: Option<ObjectParticleTextureNode<TextureList>> = particle_texture_node(
            wld,
            reference,
            0x03,
            &mut frame_ancestors,
            &mut source.issues,
        );
        if let Some(bitmap) = &bitmap
            && (bitmap.definition.filenames.len() != 1
                || bitmap.definition.filenames[0].is_empty()
                || bitmap.definition.tail.len() > 3
                || bitmap.definition.tail.iter().any(|byte| *byte != 0))
        {
            source
                .issues
                .push(ObjectParticleTextureIssue::UnsupportedLayout {
                    definition_reference: bitmap.definition_reference,
                    reason: "unproven particle bitmap layers, empty filename or extension data",
                });
        }
        source.frames.push(bitmap);
    }
    source
}
