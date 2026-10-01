//! Native shuffled-table algorithm, explicit diagnostic MSVC-style rand seed 1.
//! The real client's seed and consumption by unrelated effects remain unknown.

#[derive(Debug, Clone)]
pub struct DiagnosticRandom {
    signed: Box<[f32; 2000]>,
    unit: Box<[f32; 1000]>,
    cosine: Box<[f32; 512]>,
    tangent: Box<[f32; 128]>,
    signed_counter: u32,
    unit_counter: u32,
}

impl DiagnosticRandom {
    /// Matches the frozen native witness, not an inferred original client seed.
    pub fn seed_one() -> Self {
        let mut state = 1u32;
        let mut next = || {
            state = state.wrapping_mul(214013).wrapping_add(2531011);
            (state >> 16) & 32767
        };
        let mut shuffle = |values: &mut [f32]| {
            for i in 0..values.len() {
                let j = (f64::from(next())
                    * f64::from(f32::from_bits(0x38000100))
                    * values.len() as f64) as usize;
                // Seed 1 has no endpoint overflow in these original three loops.
                assert!(j < values.len());
                values.swap(i, j);
            }
        };
        // This table is not sampled by PoK shape 4, but its initialization draws
        // must precede the signed and unit table shuffles.
        let mut angles = [0.0; 512];
        for (i, v) in angles.iter_mut().enumerate() {
            *v = i as f32;
        }
        shuffle(&mut angles);
        let mut signed = Box::new(std::array::from_fn(|i| {
            (i as f64 * f64::from(0.001_f32) - 1.0) as f32
        }));
        shuffle(signed.as_mut_slice());
        let mut unit = Box::new(std::array::from_fn(|i| {
            (i as f64 * f64::from(0.001_f32)) as f32
        }));
        shuffle(unit.as_mut_slice());
        // Preserve the DLL's literal instead of substituting Rust's PI constant.
        #[allow(clippy::approx_constant)]
        let mut cosine = Box::new(std::array::from_fn(|i| {
            (i as f64 * 3.1415926535 / 256.0).cos() as f32
        }));
        for (index, value) in [(0, 1.0), (128, 0.0), (256, -1.0), (384, 0.0)] {
            cosine[index] = value;
        }
        let tangent = Box::new(std::array::from_fn(|i| {
            (f64::from(cosine[(i + 384) % 512]) / f64::from(cosine[i])) as f32
        }));
        Self {
            signed,
            unit,
            cosine,
            tangent,
            signed_counter: 0,
            unit_counter: 0,
        }
    }

    pub fn counters(&self) -> (u32, u32) {
        (self.signed_counter, self.unit_counter)
    }

    fn signed(&mut self) -> f32 {
        let value = self.signed[self.signed_counter as usize % 2000];
        self.signed_counter = self.signed_counter.wrapping_add(1);
        value
    }

    pub(super) fn birth(&mut self) -> (u16, f32) {
        // Seven unit draws: two size fields (one reused), axial velocity, two
        // zero velocity ranges, radial velocity, and zero angular velocity.
        let radial_index = self.unit_counter.wrapping_add(5) as usize % 1000;
        let fraction = self.unit[radial_index];
        self.unit_counter = self.unit_counter.wrapping_add(7);
        // The complete table contains accepted pairs for both possible parities.
        // A bounded full cycle therefore always finds a point in the disk.
        for _ in 0..1000 {
            let x = self.signed();
            let y = self.signed();
            if f64::from(x).powi(2) + f64::from(y).powi(2) <= 1.0 {
                return (self.azimuth(x, y), fraction);
            }
        }
        unreachable!("verified diagnostic signed table has no accepted pair")
    }

    fn azimuth(&self, x: f32, y: f32) -> u16 {
        if x == 0.0 {
            return if y >= 0.0 { 128 } else { 384 };
        }
        let slope = (f64::from(y) / f64::from(x)).abs();
        let mut lower = 0usize;
        while lower < 127 && slope >= f64::from(self.tangent[lower + 1]) {
            lower += 1;
        }
        let upper = lower + usize::from(slope != f64::from(self.tangent[lower]));
        (if x > 0.0 && y >= 0.0 {
            lower
        } else if x < 0.0 && y >= 0.0 {
            256 - upper
        } else if x < 0.0 {
            256 + lower
        } else {
            512 - upper
        }) as u16
    }

    pub(super) fn sin_cos(&self, angle: u16) -> (f32, f32) {
        (
            self.cosine[(usize::from(angle) + 384) % 512],
            self.cosine[usize::from(angle) % 512],
        )
    }
}
