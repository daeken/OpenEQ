//! Conservative, static XY lookup for zone point lights.
//!
//! Coordinates stay in EQ space: the renderer maps world `(x, -z)` back to EQ
//! `(x, y)`. Each cell preserves the source light order, so applying the existing
//! sphere test and shading to its candidates preserves accumulation order too.

// Room-sized cells avoid testing lights from neighboring buildings at Retina
// resolutions. Large zone extents automatically grow these to bound memory.
const INITIAL_CELL_SIZE: f32 = 32.;
const MAX_AXIS_CELLS: u32 = 128;
const MAX_REFERENCES: usize = 1_048_576;
const HEADER_BYTES: usize = 32;

#[derive(Debug)]
pub(crate) struct LightGrid {
    /// Little-endian WGSL storage buffer: origin `vec2<f32>`, cell size and
    /// padding, dimensions `vec2<u32>`, two padding words, then `array<u32>`.
    /// Data starts with two words per cell: offset and count. Offsets address
    /// the data array, not the whole buffer. Dimensions of zero request the
    /// original full-light scan. Disabled grids include two dummy words to meet
    /// WGSL's 40-byte minimum size (the structure has eight-byte alignment).
    pub bytes: Vec<u8>,
    pub stats: LightGridStats,
}

#[derive(Clone, Debug)]
pub(crate) struct LightGridStats {
    pub enabled: bool,
    pub light_count: usize,
    pub dimensions: [u32; 2],
    pub cell_size: f32,
    pub references: usize,
    pub max_cell_lights: u32,
    /// Includes empty cells, matching a uniform sample over the grid footprint.
    pub mean_cell_lights: f64,
    pub fallback_reason: Option<&'static str>,
}

#[derive(Clone, Copy)]
struct CellRect {
    min: [u32; 2],
    max: [u32; 2],
}

impl CellRect {
    fn cells(self, width: u32) -> impl Iterator<Item = usize> {
        (self.min[1]..=self.max[1])
            .flat_map(move |y| (self.min[0]..=self.max[0]).map(move |x| (y * width + x) as usize))
    }
}

impl LightGrid {
    /// Builds from the same packed position/radius/color records as the shader.
    /// Invalid or excessive inputs disable the grid rather than dropping lights.
    pub fn build(lights: &[[f32; 8]]) -> Self {
        if lights.is_empty() {
            return Self::disabled(0, None);
        }
        // Every light occupies at least one cell; reject before allocating any
        // per-light or per-cell data when even that minimum exceeds the budget.
        if lights.len() > MAX_REFERENCES {
            return Self::disabled(lights.len(), Some("light reference budget exceeded"));
        }
        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];
        for light in lights {
            if light.iter().any(|value| !value.is_finite()) || light[3] <= 0. {
                return Self::disabled(lights.len(), Some("invalid light record"));
            }
            for axis in 0..2 {
                min[axis] = min[axis].min(f64::from(light[axis]) - f64::from(light[3]));
                max[axis] = max[axis].max(f64::from(light[axis]) + f64::from(light[3]));
            }
        }
        if min
            .into_iter()
            .chain(max)
            .any(|n| n.abs() > f64::from(f32::MAX))
        {
            return Self::disabled(lights.len(), Some("light bounds exceed finite grid range"));
        }

        let mut cell_size = INITIAL_CELL_SIZE;
        let (origin, dimensions) = loop {
            if !cell_size.is_finite() {
                return Self::disabled(lights.len(), Some("light bounds exceed finite grid range"));
            }
            let cell = f64::from(cell_size);
            // Padding covers float rounding at the outer border as well as the
            // cell borders. Round the f32 origin outward, never into the bounds.
            let origin = min.map(|value| (((value / cell).floor() - 1.) * cell) as f32);
            let origin = origin.map(f32::next_down);
            if origin.iter().any(|value| !value.is_finite()) {
                return Self::disabled(lights.len(), Some("light bounds exceed finite grid range"));
            }
            let extent: [f64; 2] = std::array::from_fn(|axis| max[axis] - f64::from(origin[axis]));
            if extent.iter().any(|value| *value > f64::from(f32::MAX)) {
                // The shader subtracts its origin in f32. A grid whose internal
                // coordinate subtraction can overflow must use the old path.
                return Self::disabled(lights.len(), Some("light bounds exceed finite grid range"));
            }
            let dimensions = extent.map(|value| (value / cell).ceil() + 1.);
            if dimensions
                .iter()
                .all(|value| *value <= f64::from(MAX_AXIS_CELLS))
            {
                break (origin, dimensions.map(|value| value as u32));
            }
            cell_size *= 2.;
        };

        let cell_count = (dimensions[0] * dimensions[1]) as usize;
        let mut counts = vec![0_u32; cell_count];
        let mut rects = Vec::with_capacity(lights.len());
        let mut references = 0_usize;
        for light in lights {
            let range = |axis: usize, upper: bool| {
                let radius = f64::from(light[3]) * if upper { 1. } else { -1. };
                let coordinate = (f64::from(light[axis]) + radius - f64::from(origin[axis]))
                    / f64::from(cell_size);
                // One neighboring cell on each side also covers rounding of a
                // shader's distance/radius test and f32 cell coordinate math.
                let index = coordinate.floor() as i64 + if upper { 1 } else { -1 };
                index.clamp(0, i64::from(dimensions[axis]) - 1) as u32
            };
            let rect = CellRect {
                min: [range(0, false), range(1, false)],
                max: [range(0, true), range(1, true)],
            };
            let added =
                (rect.max[0] - rect.min[0] + 1) as usize * (rect.max[1] - rect.min[1] + 1) as usize;
            if added > MAX_REFERENCES - references {
                return Self::disabled(lights.len(), Some("light reference budget exceeded"));
            }
            references += added;
            for cell in rect.cells(dimensions[0]) {
                counts[cell] += 1;
            }
            rects.push(rect);
        }

        let mut data = vec![0_u32; cell_count * 2 + references];
        let mut cursors = Vec::with_capacity(cell_count);
        let mut offset = cell_count * 2;
        for (cell, &count) in counts.iter().enumerate() {
            data[cell * 2] = offset as u32;
            data[cell * 2 + 1] = count;
            cursors.push(offset);
            offset += count as usize;
        }
        // Populate in original order, including duplicates and zero-color
        // lights. This is only an acceleration structure, not a light filter.
        for (index, rect) in rects.into_iter().enumerate() {
            for cell in rect.cells(dimensions[0]) {
                data[cursors[cell]] = index as u32;
                cursors[cell] += 1;
            }
        }
        let stats = LightGridStats {
            enabled: true,
            light_count: lights.len(),
            dimensions,
            cell_size,
            references,
            max_cell_lights: counts.iter().copied().max().unwrap_or(0),
            mean_cell_lights: references as f64 / cell_count as f64,
            fallback_reason: None,
        };
        Self {
            bytes: pack(origin, cell_size, dimensions, &data),
            stats,
        }
    }

    fn disabled(light_count: usize, reason: Option<&'static str>) -> Self {
        Self {
            bytes: pack([0.; 2], INITIAL_CELL_SIZE, [0; 2], &[0, 0]),
            stats: LightGridStats {
                enabled: false,
                light_count,
                dimensions: [0; 2],
                cell_size: INITIAL_CELL_SIZE,
                references: 0,
                max_cell_lights: 0,
                mean_cell_lights: 0.,
                fallback_reason: reason,
            },
        }
    }
}

fn pack(origin: [f32; 2], cell_size: f32, dimensions: [u32; 2], data: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(HEADER_BYTES + data.len() * 4);
    for value in [
        origin[0].to_bits(),
        origin[1].to_bits(),
        cell_size.to_bits(),
        0,
        dimensions[0],
        dimensions[1],
        0,
        0,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in data {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn light(x: f32, y: f32, z: f32, radius: f32) -> [f32; 8] {
        [x, y, z, radius, 1., 0.5, 0.25, 1.]
    }

    fn word(grid: &LightGrid, offset: usize) -> u32 {
        u32::from_le_bytes(grid.bytes[offset..offset + 4].try_into().unwrap())
    }

    /// Mirrors the intended shader lookup, including f32 coordinate math.
    fn candidates(grid: &LightGrid, point: [f32; 3]) -> Vec<usize> {
        let dimensions = [word(grid, 16), word(grid, 20)];
        if dimensions.contains(&0) {
            return (0..grid.stats.light_count).collect();
        }
        let cell_size = f32::from_bits(word(grid, 8));
        let cell: [f32; 2] = std::array::from_fn(|axis| {
            ((point[axis] - f32::from_bits(word(grid, axis * 4))) / cell_size).floor()
        });
        if (0..2).any(|axis| cell[axis] < 0. || cell[axis] >= dimensions[axis] as f32) {
            return Vec::new();
        }
        let index = cell[1] as u32 * dimensions[0] + cell[0] as u32;
        let offset = word(grid, HEADER_BYTES + index as usize * 8) as usize;
        let count = word(grid, HEADER_BYTES + index as usize * 8 + 4) as usize;
        (offset..offset + count)
            .map(|i| word(grid, HEADER_BYTES + i * 4) as usize)
            .collect()
    }

    fn affects(light: &[f32; 8], point: [f32; 3]) -> bool {
        let delta: [f32; 3] = std::array::from_fn(|axis| light[axis] - point[axis]);
        let distance = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
        distance < light[3]
    }

    fn assert_matches(grid: &LightGrid, lights: &[[f32; 8]], point: [f32; 3]) {
        let brute: Vec<_> = (0..lights.len())
            .filter(|&i| affects(&lights[i], point))
            .collect();
        let indexed: Vec<_> = candidates(grid, point)
            .into_iter()
            .filter(|&i| affects(&lights[i], point))
            .collect();
        assert_eq!(indexed, brute, "point={point:?}, stats={:?}", grid.stats);
    }

    #[test]
    fn empty_grid_has_complete_disabled_buffer() {
        let grid = LightGrid::build(&[]);
        assert!(!grid.stats.enabled);
        assert_eq!(grid.bytes.len(), HEADER_BYTES + 8);
        assert_eq!(grid.stats.dimensions, [0; 2]);
        assert_eq!(grid.stats.max_cell_lights, 0);
        assert_eq!(grid.stats.mean_cell_lights, 0.);
        assert_eq!(grid.stats.fallback_reason, None);
        assert_matches(&grid, &[], [-128., 128., 0.]);
    }

    #[test]
    fn packed_offsets_counts_and_order_are_consistent() {
        let lights = [light(0., 0., 0., 24.); 3];
        let grid = LightGrid::build(&lights);
        assert!(grid.stats.enabled);
        assert_eq!(candidates(&grid, [0.; 3]), [0, 1, 2]);
        let cells = (grid.stats.dimensions[0] * grid.stats.dimensions[1]) as usize;
        let mut references = 0;
        for cell in 0..cells {
            let offset = word(&grid, HEADER_BYTES + cell * 8) as usize;
            let count = word(&grid, HEADER_BYTES + cell * 8 + 4) as usize;
            assert_eq!(offset, cells * 2 + references);
            assert!(HEADER_BYTES + (offset + count) * 4 <= grid.bytes.len());
            references += count;
        }
        assert_eq!(references, grid.stats.references);
        assert_eq!(grid.stats.max_cell_lights, 3);
        assert_eq!(
            grid.stats.mean_cell_lights,
            references as f64 / cells as f64
        );
        assert_matches(&grid, &lights, [1e6, -1e6, 0.]);
        assert!(candidates(&grid, [1e6, -1e6, 0.]).is_empty());
    }

    #[test]
    fn negative_coordinates_and_float_cell_boundaries_keep_lights() {
        let lights = [
            light(-256., -128., 0., 128.),
            light(-0.001, 0., 0., 24.),
            light(128., -256., 30., 256.),
        ];
        let grid = LightGrid::build(&lights);
        for x in [-512_f32, -384., -256., -128., 0., 128., 256., 384.] {
            for y in [-512_f32, -256., -128., 0., 128., 256.] {
                for xx in [x.next_down(), x, x.next_up()] {
                    for yy in [y.next_down(), y, y.next_up()] {
                        assert_matches(&grid, &lights, [xx, yy, 0.]);
                    }
                }
            }
        }
    }

    #[test]
    fn overlapping_lights_match_brute_force_at_deterministic_points() {
        let lights: Vec<_> = (0..57)
            .map(|i| {
                light(
                    (i * 137 % 1301) as f32 - 650.,
                    (i * 271 % 977) as f32 - 488.,
                    (i * 31 % 101) as f32 - 50.,
                    (24 + i * 23 % 451) as f32,
                )
            })
            .collect();
        let grid = LightGrid::build(&lights);
        assert!(grid.stats.enabled);
        let mut random = 0x51ed_270b_u32;
        let mut coordinate = |scale: f32| {
            random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (random as f64 / f64::from(u32::MAX) - 0.5) as f32 * scale
        };
        for _ in 0..10_000 {
            assert_matches(
                &grid,
                &lights,
                [coordinate(3000.), coordinate(2500.), coordinate(700.)],
            );
        }
        for source in &lights {
            for axis in 0..2 {
                for sign in [-1., 1.] {
                    let mut point = [source[0], source[1], source[2]];
                    point[axis] += sign * source[3];
                    for value in [point[axis].next_down(), point[axis], point[axis].next_up()] {
                        point[axis] = value;
                        assert_matches(&grid, &lights, point);
                    }
                }
            }
        }
    }

    #[test]
    fn huge_radius_adapts_cell_size_without_losing_lights() {
        let lights = [light(-1e6, 2e6, 0., 1e6), light(1e6, -1e6, 0., 24.)];
        let grid = LightGrid::build(&lights);
        assert!(grid.stats.enabled);
        assert!(grid.stats.cell_size > INITIAL_CELL_SIZE);
        assert!(grid.stats.dimensions.iter().all(|&n| n <= MAX_AXIS_CELLS));
        for x in -5..=5 {
            for y in -5..=5 {
                assert_matches(
                    &grid,
                    &lights,
                    [x as f32 * 499_999., y as f32 * 499_999., 0.],
                );
            }
        }
        assert_matches(&grid, &lights, [1e6, -1e6, 0.]);
    }

    #[test]
    fn excessive_overlap_falls_back_without_truncating_lights() {
        let lights = vec![light(0., 0., 0., 1e6); 300];
        let grid = LightGrid::build(&lights);
        assert!(!grid.stats.enabled);
        assert_eq!(
            grid.stats.fallback_reason,
            Some("light reference budget exceeded")
        );
        assert_eq!(grid.bytes.len(), HEADER_BYTES + 8);
        assert_matches(&grid, &lights, [0.; 3]);
        assert_eq!(candidates(&grid, [0.; 3]).len(), lights.len());
    }

    #[test]
    fn invalid_and_extreme_inputs_fall_back_safely() {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            for axis in 0..8 {
                let mut source = light(0., 0., 0., 24.);
                source[axis] = value;
                let grid = LightGrid::build(&[source]);
                assert!(!grid.stats.enabled);
                assert_eq!(grid.stats.fallback_reason, Some("invalid light record"));
            }
        }
        for radius in [0., -1.] {
            assert!(!LightGrid::build(&[light(0., 0., 0., radius)]).stats.enabled);
        }
        for lights in [
            vec![light(f32::MAX, 0., 0., f32::MAX)],
            vec![light(-f32::MAX, 0., 0., 24.)],
            vec![
                light(-f32::MAX / 2., 0., 0., 24.),
                light(f32::MAX / 2., 0., 0., 24.),
            ],
        ] {
            let grid = LightGrid::build(&lights);
            assert!(!grid.stats.enabled);
            assert_eq!(grid.stats.dimensions, [0; 2]);
            assert_eq!(grid.bytes.len(), HEADER_BYTES + 8);
            assert_matches(&grid, &lights, [0.; 3]);
        }
    }
}
