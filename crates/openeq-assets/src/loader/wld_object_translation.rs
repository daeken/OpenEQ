//! Bounded native five-frame translation reduction. See WLD_OBJECT_TRANSLATION.md.
use super::{Frame, Result, invalid};

#[derive(Debug)]
pub(super) struct FiveFrameTranslations {
    translations: [[f32; 3]; 6],
    pub(super) omitted: usize,
}

impl FiveFrameTranslations {
    pub(super) fn new(frames: &[Frame]) -> Result<Self> {
        if frames.len() != 5
            || frames
                .iter()
                .any(|frame| frame.rotation != frames[0].rotation || frame.scale != frames[0].scale)
        {
            return Err(invalid(
                "changing object translation requires five frames with constant rotation and scale",
            ));
        }
        let translations = std::array::from_fn(|index| frames[index % 5].translation);
        if translations.iter().flatten().any(|value| {
            let packed = *value * 256.;
            !packed.is_finite() || !(-32768. ..=32767.).contains(&packed) || packed.fract() != 0.
        }) {
            return Err(invalid("unproven object translation encoding"));
        }
        // EQ first removes runs whose adjacent components differ by at most
        // 0.0001. Distinct packed positions differ by at least 1/256, so this
        // gate proves that all six keys, including closure, reach D3DX intact.
        if translations.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid("unproven repeated object translation keys"));
        }
        let mut scores = [0.; 4];
        for index in 1..5 {
            let mut score = 0f64;
            for axis in (0..3).rev() {
                // Packed positions and their midpoint/difference are exact
                // in f32. Require every square and partial sum to be exact
                // too, excluding x87 precision-dependent removal rankings.
                let midpoint = (f64::from(translations[index - 1][axis])
                    + f64::from(translations[index + 1][axis]))
                    * 0.5;
                let difference = f64::from(translations[index][axis]) - midpoint;
                let square = difference * difference;
                score += square;
                if f64::from(square as f32) != square || f64::from(score as f32) != score {
                    return Err(invalid("inexact object translation reduction score"));
                }
            }
            scores[index - 1] = score as f32;
        }
        let omitted = (1..5)
            .min_by(|&a, &b| scores[a - 1].total_cmp(&scores[b - 1]))
            .unwrap();
        if (1..5).any(|index| index != omitted && scores[index - 1] == scores[omitted - 1]) {
            return Err(invalid("ambiguous object translation reduction"));
        }
        // Lossiness 0.1 retains five of six keys. The first minimum squared
        // interpolation error is the only discarded translation key; rotation
        // has its own independent reduction and must not supply this index.
        Ok(Self {
            translations,
            omitted,
        })
    }

    pub(super) fn sample(&self, phase: u128, interval: u32) -> [f32; 3] {
        let index = (phase / u128::from(interval)) as usize;
        let left = if index == self.omitted {
            index - 1
        } else {
            index
        };
        let right = left + if left + 1 == self.omitted { 2 } else { 1 };
        let fraction = f64::from(phase as u32 - left as u32 * interval)
            / f64::from((right - left) as u32 * interval);
        std::array::from_fn(|axis| {
            let a = f64::from(self.translations[left][axis]);
            let b = f64::from(self.translations[right][axis]);
            (a + (b - a) * fraction) as f32
        })
    }
}
