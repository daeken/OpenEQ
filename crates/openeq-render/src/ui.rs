//! Screen-space overlay for the original client's XML UI draw list.
//! Render after zone lighting with no depth attachment. Frame geometry is in
//! logical pixels; prepare_scaled supplies the display pixel density.

use bytemuck::{Pod, Zeroable};
use font8x8::UnicodeFonts;
use openeq_ui::{
    Color, DrawCommand, HitTarget, Rect, ScrollThumb, TextAlign, TextScrollMetrics, UiFrame,
};
use std::{
    collections::HashMap,
    ops::Range,
    path::{Path, PathBuf},
};
use wgpu::util::DeviceExt;

const GLYPH_ATLAS_SIZE: u32 = 1024;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

struct Texture {
    bind_group: wgpu::BindGroup,
    size: [u32; 2],
    _texture: wgpu::Texture,
}
struct Batch {
    texture: usize,
    clip: [u32; 4],
    vertices: Range<u32>,
}
#[derive(Clone, Copy)]
struct Glyph {
    source: Rect,
    offset: [f32; 2],
    advance: f32,
}

/// Texture/glyph caches survive successive frames. Preparation uploads newly
/// encountered assets and replaces the vertex buffer; render only issues draws.
pub struct UiRenderer {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    textures: Vec<Texture>,
    texture_ids: HashMap<PathBuf, usize>,
    font: Option<fontdue::Font>,
    glyphs: HashMap<(char, u32), Glyph>,
    glyph_cursor: [u32; 2],
    glyph_row_height: u32,
    vertices: wgpu::Buffer,
    batches: Vec<Batch>,
    scale: f32,
    link_hits: Vec<HitTarget>,
    text_scroll_metrics: Vec<TextScrollMetrics>,
}

impl UiRenderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("UI image layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("UI sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("UI pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("UI shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ui.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("UI pipeline"), layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), buffers: &[wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4] }], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs_main"), targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
            primitive: Default::default(), depth_stencil: None, multisample: Default::default(), multiview_mask: None, cache: None,
        });
        let white = texture(
            device,
            queue,
            &layout,
            &sampler,
            "UI white",
            [1, 1],
            &[255; 4],
        );
        let glyph_atlas = texture(
            device,
            queue,
            &layout,
            &sampler,
            "UI font atlas",
            [GLYPH_ATLAS_SIZE; 2],
            &vec![0; (GLYPH_ATLAS_SIZE * GLYPH_ATLAS_SIZE * 4) as usize],
        );
        let font = load_font();
        if font.is_none() {
            tracing::warn!(
                "No UI font found; using built-in bitmap font. Set OPENEQ_UI_FONT to a TTF file for smoother text."
            );
        }
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("UI vertices"),
            size: 4,
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            layout,
            sampler,
            textures: vec![white, glyph_atlas],
            texture_ids: HashMap::new(),
            font,
            glyphs: HashMap::new(),
            glyph_cursor: [1, 1],
            glyph_row_height: 0,
            vertices,
            batches: Vec::new(),
            scale: 1.,
            link_hits: Vec::new(),
            text_scroll_metrics: Vec::new(),
        }
    }

    /// One physical pixel per logical pixel; use prepare_scaled on HiDPI windows.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &UiFrame,
        size: [u32; 2],
    ) {
        self.prepare_scaled(device, queue, frame, size, 1.);
    }

    /// `size` is the physical render target; frame coordinates and hit regions
    /// stay in logical pixels. Text is rasterized at the physical pixel density.
    pub fn prepare_scaled(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &UiFrame,
        size: [u32; 2],
        scale: f32,
    ) {
        self.scale = if scale.is_finite() && scale > 0. {
            scale.clamp(0.25, 8.)
        } else {
            1.
        };
        self.batches.clear();
        self.link_hits.clear();
        self.text_scroll_metrics.clear();
        if size[0] == 0 || size[1] == 0 {
            return;
        }
        let mut vertices = Vec::new();
        for command in &frame.commands {
            match command {
                DrawCommand::Line {
                    from,
                    to,
                    width,
                    clip,
                    color,
                } => {
                    if !from
                        .iter()
                        .chain(to)
                        .chain(std::iter::once(width))
                        .all(|n| n.is_finite())
                        || *width <= 0.
                    {
                        continue;
                    }
                    let dx = to[0] - from[0];
                    let dy = to[1] - from[1];
                    let length = dx.hypot(dy);
                    if length <= f32::EPSILON {
                        continue;
                    }
                    let nx = -dy / length * width * 0.5;
                    let ny = dx / length * width * 0.5;
                    self.quad_points(
                        &mut vertices,
                        [
                            [from[0] + nx, from[1] + ny],
                            [from[0] - nx, from[1] - ny],
                            [to[0] + nx, to[1] + ny],
                            [to[0] - nx, to[1] - ny],
                        ],
                        *clip,
                        [0., 0., 1., 1.],
                        *color,
                        0,
                        size,
                    );
                }

                DrawCommand::TextArea {
                    id,
                    rect,
                    clip,
                    text,
                    font,
                    color,
                    scroll_rows,
                    thumb,
                } => {
                    let px = (font_size(*font) as f32 * self.scale).round().max(1.) as u32;
                    let metrics = self
                        .font
                        .as_ref()
                        .and_then(|font| font.horizontal_line_metrics(px as f32));
                    let line_height = metrics
                        .map_or(px as f32 * 1.2, |metrics| metrics.new_line_size)
                        .ceil()
                        / self.scale;
                    let ascent = metrics.map_or(px as f32, |metrics| metrics.ascent) / self.scale;
                    let rows = self.text_row_ranges(text, px, rect.width.max(1.));
                    let visible_rows = (rect.height / line_height).floor().max(0.) as usize;
                    let first_row = (*scroll_rows).min(rows.len().saturating_sub(visible_rows));
                    let metrics = TextScrollMetrics {
                        id: id.clone(),
                        total_rows: rows.len(),
                        visible_rows,
                        first_row,
                    };
                    let clip = clip.intersect(*rect);
                    for (row, range) in rows.iter().skip(first_row).take(visible_rows).enumerate() {
                        let baseline = rect.y + row as f32 * line_height + ascent;
                        let mut x = rect.x;
                        for ch in text[range.clone()].chars() {
                            let glyph = self.glyph(queue, ch, px);
                            if !glyph.source.is_empty() {
                                let bounds = Rect::new(
                                    (x * self.scale + glyph.offset[0]).round() / self.scale,
                                    (baseline * self.scale + glyph.offset[1]).round() / self.scale,
                                    glyph.source.width / self.scale,
                                    glyph.source.height / self.scale,
                                );
                                let atlas = GLYPH_ATLAS_SIZE as f32;
                                self.quad(
                                    &mut vertices,
                                    bounds,
                                    clip,
                                    [
                                        glyph.source.x / atlas,
                                        glyph.source.y / atlas,
                                        glyph.source.right() / atlas,
                                        glyph.source.bottom() / atlas,
                                    ],
                                    *color,
                                    1,
                                    size,
                                );
                            }
                            x += glyph.advance / self.scale;
                        }
                    }
                    if let Some(thumb) = thumb {
                        self.scroll_thumb(device, queue, &mut vertices, thumb, &metrics, size);
                    }
                    self.text_scroll_metrics.push(metrics);
                }

                DrawCommand::TextLog {
                    rect,
                    clip,
                    lines,
                    font,
                    scroll_rows,
                } => {
                    let px = (font_size(*font) as f32 * self.scale).round().max(1.) as u32;
                    let line_height = self
                        .font
                        .as_ref()
                        .and_then(|font| font.horizontal_line_metrics(px as f32))
                        .map_or(px as f32 * 1.2, |metrics| metrics.new_line_size)
                        .ceil()
                        / self.scale;
                    let ascent = self
                        .font
                        .as_ref()
                        .and_then(|font| font.horizontal_line_metrics(px as f32))
                        .map_or(px as f32, |metrics| metrics.ascent)
                        / self.scale;
                    let visible = (rect.height / line_height).floor().max(0.) as usize;
                    let mut rows = Vec::new();
                    for (index, line) in lines.iter().enumerate() {
                        for range in self.text_row_ranges(&line.text, px, rect.width) {
                            rows.push((index, range));
                        }
                    }
                    let end = rows
                        .len()
                        .saturating_sub((*scroll_rows).min(rows.len().saturating_sub(visible)));
                    let start = end.saturating_sub(visible);
                    let top = rect.bottom() - (end - start) as f32 * line_height;
                    for (row, (index, range)) in rows[start..end].iter().enumerate() {
                        let line = &lines[*index];
                        let text = &line.text[range.clone()];
                        let row_y = top + row as f32 * line_height;
                        let baseline = row_y + ascent;
                        let mut x = rect.x;
                        let mut runs = Vec::<(u64, Rect)>::new();
                        for (offset, ch) in text.char_indices() {
                            let byte = range.start + offset;
                            let link = line.links.iter().find(|link| {
                                link.range.start < link.range.end
                                    && link.range.end <= line.text.len()
                                    && line.text.is_char_boundary(link.range.start)
                                    && line.text.is_char_boundary(link.range.end)
                                    && link.range.contains(&byte)
                            });
                            let color = if link.is_some() {
                                [130, 205, 255, 255]
                            } else {
                                line.color
                            };
                            let glyph = self.glyph(queue, ch, px);
                            let advance = glyph.advance / self.scale;
                            if let Some(link) = link {
                                if let Some((_, bounds)) = runs.last_mut().filter(|(id, bounds)| {
                                    *id == link.id && (bounds.right() - x).abs() < 0.01
                                }) {
                                    bounds.width += advance;
                                } else {
                                    runs.push((link.id, Rect::new(x, row_y, advance, line_height)));
                                }
                            }
                            if !glyph.source.is_empty() {
                                let bounds = Rect::new(
                                    (x * self.scale + glyph.offset[0]).round() / self.scale,
                                    (baseline * self.scale + glyph.offset[1]).round() / self.scale,
                                    glyph.source.width / self.scale,
                                    glyph.source.height / self.scale,
                                );
                                let atlas = GLYPH_ATLAS_SIZE as f32;
                                self.quad(
                                    &mut vertices,
                                    bounds,
                                    *clip,
                                    [
                                        glyph.source.x / atlas,
                                        glyph.source.y / atlas,
                                        glyph.source.right() / atlas,
                                        glyph.source.bottom() / atlas,
                                    ],
                                    color,
                                    1,
                                    size,
                                );
                            }
                            x += advance;
                        }
                        for (id, bounds) in runs {
                            let clipped = bounds.intersect(*clip).intersect(*rect);
                            if clipped.is_empty() {
                                continue;
                            }
                            self.quad(
                                &mut vertices,
                                Rect::new(bounds.x, baseline + 1., bounds.width, 1. / self.scale),
                                *clip,
                                [0., 0., 1., 1.],
                                [130, 205, 255, 255],
                                0,
                                size,
                            );
                            let item = format!("chat:link:{id}");
                            self.link_hits.push(HitTarget {
                                window_id: None,
                                screen_id: item.clone(),
                                item,
                                kind: "ChatLink".into(),
                                rect: clipped,
                                enabled: true,
                                tooltip: None,
                            });
                        }
                    }
                }

                DrawCommand::Fill { rect, clip, color } => self.quad(
                    &mut vertices,
                    *rect,
                    *clip,
                    [0., 0., 1., 1.],
                    *color,
                    0,
                    size,
                ),
                DrawCommand::Image {
                    rect,
                    clip,
                    texture,
                    source,
                    tint,
                    ..
                } => {
                    let id = self.load_texture(device, queue, texture);
                    let [width, height] = self.textures[id].size.map(|n| n as f32);
                    let source = if source.width == 0. || source.height == 0. {
                        Rect::new(0., 0., width, height)
                    } else {
                        *source
                    };
                    let uv = [
                        source.x / width,
                        source.y / height,
                        source.right() / width,
                        source.bottom() / height,
                    ];
                    self.quad(&mut vertices, *rect, *clip, uv, *tint, id, size);
                }
                DrawCommand::Text {
                    rect,
                    clip,
                    text,
                    font,
                    color,
                    align,
                    vertical_center,
                    wrap,
                } => {
                    let px = (font_size(*font) as f32 * self.scale).round().max(1.) as u32;
                    let lines =
                        self.text_lines(text, px, if *wrap { Some(rect.width) } else { None });
                    let line_height = self
                        .font
                        .as_ref()
                        .and_then(|font| font.horizontal_line_metrics(px as f32))
                        .map_or(px as f32 * 1.2, |metrics| metrics.new_line_size)
                        / self.scale;
                    let ascent = self
                        .font
                        .as_ref()
                        .and_then(|font| font.horizontal_line_metrics(px as f32))
                        .map_or(px as f32, |metrics| metrics.ascent)
                        / self.scale;
                    let top = rect.y
                        + if *vertical_center {
                            ((rect.height - line_height * lines.len() as f32) / 2.).max(0.)
                        } else {
                            0.
                        };
                    for (line_number, line) in lines.iter().enumerate() {
                        let width = self.text_width(line, px);
                        let mut x = rect.x
                            + match align {
                                TextAlign::Left => 0.,
                                TextAlign::Center => (rect.width - width) * 0.5,
                                TextAlign::Right => rect.width - width,
                            };
                        let baseline = top + line_number as f32 * line_height + ascent;
                        for ch in line.chars() {
                            let glyph = self.glyph(queue, ch, px);
                            if !glyph.source.is_empty() {
                                let bounds = Rect::new(
                                    (x * self.scale + glyph.offset[0]).round() / self.scale,
                                    (baseline * self.scale + glyph.offset[1]).round() / self.scale,
                                    glyph.source.width / self.scale,
                                    glyph.source.height / self.scale,
                                );
                                let atlas = GLYPH_ATLAS_SIZE as f32;
                                self.quad(
                                    &mut vertices,
                                    bounds,
                                    *clip,
                                    [
                                        glyph.source.x / atlas,
                                        glyph.source.y / atlas,
                                        glyph.source.right() / atlas,
                                        glyph.source.bottom() / atlas,
                                    ],
                                    *color,
                                    1,
                                    size,
                                );
                            }
                            x += glyph.advance / self.scale;
                        }
                    }
                }
            }
        }
        if !vertices.is_empty() {
            self.vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("UI vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        }
    }

    /// Link rectangles from the last prepare, in logical pixels. The application
    /// must gate these by the topmost chat-log window hit before dispatching.
    pub fn link_hits(&self) -> &[HitTarget] {
        &self.link_hits
    }

    pub fn text_scroll_metrics(&self) -> &[TextScrollMetrics] {
        &self.text_scroll_metrics
    }

    fn scroll_thumb(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vertices: &mut Vec<Vertex>,
        thumb: &ScrollThumb,
        metrics: &TextScrollMetrics,
        size: [u32; 2],
    ) {
        let track = thumb.track;
        if track.is_empty() {
            return;
        }
        let top_height = thumb.images[0]
            .as_ref()
            .map_or(0., |image| image.source.height);
        let bottom_height = thumb.images[2]
            .as_ref()
            .map_or(0., |image| image.source.height);
        let fraction = metrics.visible_rows as f32 / metrics.total_rows.max(1) as f32;
        let height = (track.height * fraction)
            .max((top_height + bottom_height).max(8.))
            .min(track.height);
        let offset = if metrics.max_scroll() == 0 {
            0.
        } else {
            metrics.first_row as f32 / metrics.max_scroll() as f32
        };
        let bounds = Rect::new(
            track.x,
            track.y + (track.height - height) * offset,
            track.width,
            height,
        );
        self.quad(
            vertices,
            bounds,
            thumb.clip,
            [0., 0., 1., 1.],
            [153, 139, 100, 255],
            0,
            size,
        );
        let top_height = top_height.min(height * 0.5);
        let bottom_height = bottom_height.min(height * 0.5);
        let parts = [
            Rect::new(bounds.x, bounds.y, bounds.width, top_height),
            Rect::new(
                bounds.x,
                bounds.y + top_height,
                bounds.width,
                (height - top_height - bottom_height).max(0.),
            ),
            Rect::new(
                bounds.x,
                bounds.bottom() - bottom_height,
                bounds.width,
                bottom_height,
            ),
        ];
        for (image, rect) in thumb.images.iter().zip(parts) {
            if let Some(image) = image {
                let id = self.load_texture(device, queue, &image.texture);
                let [width, height] = self.textures[id].size.map(|n| n as f32);
                let source = image.source;
                self.quad(
                    vertices,
                    rect,
                    thumb.clip,
                    [
                        source.x / width,
                        source.y / height,
                        source.right() / width,
                        source.bottom() / height,
                    ],
                    [255; 4],
                    id,
                    size,
                );
            }
        }
    }

    pub fn render(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.batches.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        for batch in &self.batches {
            let [x, y, width, height] = batch.clip;
            pass.set_scissor_rect(x, y, width, height);
            pass.set_bind_group(0, &self.textures[batch.texture].bind_group, &[]);
            pass.draw(batch.vertices.clone(), 0..1);
        }
    }

    fn load_texture(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, path: &Path) -> usize {
        if let Some(id) = self.texture_ids.get(path) {
            return *id;
        }
        let loaded = std::fs::read(path)
            .map_err(|error| error.to_string())
            .and_then(|bytes| {
                // Some files named .tga contain DDS. Sniff known formats first;
                // true TGA has no magic number, so use the extension as fallback.
                openeq_assets::texture::Texture::decode(&path.to_string_lossy(), &bytes)
                    .map(|texture| (texture.width, texture.height, texture.rgba))
                    .or_else(|_| {
                        image::load_from_memory_with_format(&bytes, image::ImageFormat::Tga)
                            .map(|image| {
                                let image = image.to_rgba8();
                                (image.width(), image.height(), image.into_raw())
                            })
                            .map_err(|error| error.to_string())
                    })
            });
        let id = match loaded {
            Ok((width, height, rgba))
                if width > 0
                    && height > 0
                    && width <= device.limits().max_texture_dimension_2d
                    && height <= device.limits().max_texture_dimension_2d =>
            {
                let id = self.textures.len();
                self.textures.push(texture(
                    device,
                    queue,
                    &self.layout,
                    &self.sampler,
                    &path.to_string_lossy(),
                    [width, height],
                    &rgba,
                ));
                id
            }
            Ok(_) => {
                tracing::warn!(path = %path.display(), "invalid UI texture dimensions");
                0
            }
            Err(error) => {
                tracing::warn!(path = %path.display(), %error, "could not load UI texture");
                0
            }
        };
        self.texture_ids.insert(path.to_owned(), id);
        id
    }

    fn text_width(&self, text: &str, px: u32) -> f32 {
        text.chars().map(|ch| self.advance(ch, px)).sum()
    }
    fn advance(&self, ch: char, px: u32) -> f32 {
        self.font
            .as_ref()
            .map_or((px * 3 / 4).max(8) as f32, |font| {
                font.metrics(ch, px as f32).advance_width
            })
            / self.scale
    }
    fn text_row_ranges(&self, text: &str, px: u32, limit: f32) -> Vec<Range<usize>> {
        let mut rows = Vec::new();
        let mut paragraph_start = 0;
        for paragraph in text.split('\n') {
            let mut start = paragraph_start;
            let mut end = start;
            let mut width = 0.;
            let mut word_start = paragraph_start;
            for word in paragraph.split_inclusive(' ') {
                if end > start && width + self.text_width(word.trim_end(), px) > limit {
                    rows.push(start..start + text[start..end].trim_end().len());
                    start = word_start;
                    end = start;
                    width = 0.;
                }
                for (offset, ch) in word.char_indices() {
                    let byte = word_start + offset;
                    let advance = self.advance(ch, px);
                    if end > start && width + advance > limit {
                        rows.push(start..end);
                        start = byte;
                        width = 0.;
                    }
                    end = byte + ch.len_utf8();
                    width += advance;
                }
                word_start += word.len();
            }
            rows.push(start..start + text[start..end].trim_end().len());
            paragraph_start += paragraph.len() + 1;
        }
        rows
    }

    fn text_lines(&self, text: &str, px: u32, max_width: Option<f32>) -> Vec<String> {
        let mut lines = Vec::new();
        for paragraph in text.split('\n') {
            let Some(limit) = max_width else {
                lines.push(paragraph.to_owned());
                continue;
            };
            let mut line = String::new();
            let mut line_width = 0.;
            for word in paragraph.split_inclusive(' ') {
                if !line.is_empty() && line_width + self.text_width(word.trim_end(), px) > limit {
                    lines.push(line.trim_end().to_owned());
                    line.clear();
                    line_width = 0.;
                }
                // Keep wrapping linear in the log length: do not remeasure the
                // entire growing line for every character in combat scrollback.
                for ch in word.chars() {
                    let advance = self.advance(ch, px);
                    if !line.is_empty() && line_width + advance > limit {
                        lines.push(std::mem::take(&mut line));
                        line_width = 0.;
                    }
                    line.push(ch);
                    line_width += advance;
                }
            }
            lines.push(line.trim_end().to_owned());
        }
        lines
    }

    fn glyph(&mut self, queue: &wgpu::Queue, ch: char, px: u32) -> Glyph {
        if let Some(glyph) = self.glyphs.get(&(ch, px)) {
            return *glyph;
        }
        let (width, height, offset, advance, coverage) = if let Some(font) = &self.font {
            let (metrics, coverage) = font.rasterize(ch, px as f32);
            (
                metrics.width as u32,
                metrics.height as u32,
                [
                    metrics.xmin as f32,
                    -(metrics.height as f32 + metrics.ymin as f32),
                ],
                metrics.advance_width,
                coverage,
            )
        } else {
            let bitmap = font8x8::BASIC_FONTS
                .get(ch)
                .or_else(|| font8x8::BASIC_FONTS.get('?'))
                .unwrap_or([0; 8]);
            let width = (px * 3 / 4).max(8);
            let height = px.max(8);
            let mut coverage = Vec::with_capacity((width * height) as usize);
            for y in 0..height {
                for x in 0..width {
                    coverage.push(
                        if bitmap[(y * 8 / height) as usize] & (1 << (x * 8 / width)) != 0 {
                            255
                        } else {
                            0
                        },
                    );
                }
            }
            (
                width,
                height,
                [0., -(height as f32)],
                width as f32,
                coverage,
            )
        };
        if width == 0 || height == 0 {
            let glyph = Glyph {
                source: Rect::default(),
                offset,
                advance,
            };
            self.glyphs.insert((ch, px), glyph);
            return glyph;
        }
        if self.glyph_cursor[0] + width + 1 > GLYPH_ATLAS_SIZE {
            self.glyph_cursor[0] = 1;
            self.glyph_cursor[1] += self.glyph_row_height + 2;
            self.glyph_row_height = 0;
        }
        if self.glyph_cursor[1] + height + 1 > GLYPH_ATLAS_SIZE {
            // Keep prior frame UVs valid. This atlas holds thousands of typical
            // UI glyphs; avoid overwriting it if a very large alphabet fills it.
            return Glyph {
                source: Rect::default(),
                offset,
                advance,
            };
        }
        let [x, y] = self.glyph_cursor;
        let mut rgba = Vec::with_capacity(coverage.len() * 4);
        for alpha in coverage {
            rgba.extend_from_slice(&[255, 255, 255, alpha]);
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.textures[1]._texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.glyph_cursor[0] += width + 2;
        self.glyph_row_height = self.glyph_row_height.max(height);
        let glyph = Glyph {
            source: Rect::new(x as f32, y as f32, width as f32, height as f32),
            offset,
            advance,
        };
        self.glyphs.insert((ch, px), glyph);
        glyph
    }

    #[allow(clippy::too_many_arguments)]
    fn quad(
        &mut self,
        vertices: &mut Vec<Vertex>,
        rect: Rect,
        clip: Rect,
        uv: [f32; 4],
        color: Color,
        texture: usize,
        size: [u32; 2],
    ) {
        if rect.is_empty() {
            return;
        }
        self.quad_points(
            vertices,
            [
                [rect.x, rect.y],
                [rect.x, rect.bottom()],
                [rect.right(), rect.y],
                [rect.right(), rect.bottom()],
            ],
            clip,
            uv,
            color,
            texture,
            size,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn quad_points(
        &mut self,
        vertices: &mut Vec<Vertex>,
        points: [[f32; 2]; 4],
        clip: Rect,
        uv: [f32; 4],
        color: Color,
        texture: usize,
        size: [u32; 2],
    ) {
        let physical_clip = Rect::new(
            clip.x * self.scale,
            clip.y * self.scale,
            clip.width * self.scale,
            clip.height * self.scale,
        );
        let Some(scissor) = scissor(physical_clip, size) else {
            return;
        };
        if color[3] == 0 {
            return;
        }
        let [a, b, c, d] = points.map(|[x, y]| {
            [
                x * self.scale / size[0] as f32 * 2. - 1.,
                1. - y * self.scale / size[1] as f32 * 2.,
            ]
        });
        // Image textures are sRGB; vertex colors must enter the shader in linear
        // space as well or UI tints become washed out on an sRGB surface.
        let color = [
            linear(color[0]),
            linear(color[1]),
            linear(color[2]),
            color[3] as f32 / 255.,
        ];
        let [u0, v0, u1, v1] = uv;
        let begin = vertices.len() as u32;
        for (position, uv) in [
            (a, [u0, v0]),
            (b, [u0, v1]),
            (c, [u1, v0]),
            (c, [u1, v0]),
            (b, [u0, v1]),
            (d, [u1, v1]),
        ] {
            vertices.push(Vertex {
                position,
                uv,
                color,
            });
        }
        let end = vertices.len() as u32;
        if let Some(batch) = self
            .batches
            .last_mut()
            .filter(|batch| batch.texture == texture && batch.clip == scissor)
        {
            batch.vertices.end = end;
        } else {
            self.batches.push(Batch {
                texture,
                clip: scissor,
                vertices: begin..end,
            });
        }
    }
}

fn texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    label: &str,
    size: [u32; 2],
    rgba: &[u8],
) -> Texture {
    let [width, height] = size;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&Default::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    Texture {
        bind_group,
        size,
        _texture: texture,
    }
}

fn font_size(eq_font: u32) -> u32 {
    [10, 11, 12, 14, 16, 20, 24, 28][eq_font.min(7) as usize]
}
fn linear(value: u8) -> f32 {
    let value = value as f32 / 255.;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}
fn scissor(rect: Rect, size: [u32; 2]) -> Option<[u32; 4]> {
    if ![rect.x, rect.y, rect.width, rect.height]
        .iter()
        .all(|v| v.is_finite())
    {
        return None;
    }
    let x = rect.x.floor().clamp(0., size[0] as f32) as u32;
    let y = rect.y.floor().clamp(0., size[1] as f32) as u32;
    let right = rect.right().ceil().clamp(0., size[0] as f32) as u32;
    let bottom = rect.bottom().ceil().clamp(0., size[1] as f32) as u32;
    if right <= x || bottom <= y {
        None
    } else {
        Some([x, y, right - x, bottom - y])
    }
}
fn load_font() -> Option<fontdue::Font> {
    let mut paths = Vec::new();
    if let Some(path) = std::env::var_os("OPENEQ_UI_FONT") {
        paths.push(PathBuf::from(path));
    }
    paths.extend(
        [
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/System/Library/Fonts/Helvetica.ttc",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
            "C:\\Windows\\Fonts\\arial.ttf",
        ]
        .map(PathBuf::from),
    );
    paths.into_iter().find_map(|path| {
        std::fs::read(&path).ok().and_then(|bytes| {
            fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_area_scroll_wrap_thumb_and_clip_share_measured_rows() {
        for scale in [1., 2.] {
            let mut renderer =
                crate::Renderer::new_headless((128. * scale) as u32, (96. * scale) as u32).unwrap();
            let text =
                "First row\n\nCafé 雪 with words and anunbrokenwordthatmustwrapcorrectly\nLast row";
            let make_frame = |scroll_rows, width| UiFrame {
                commands: vec![DrawCommand::TextArea {
                    id: "test".into(),
                    rect: Rect::new(8., 10., width, 40.),
                    clip: Rect::new(10., 12., 90., 28.),
                    text: text.into(),
                    font: 2,
                    color: [255, 0, 0, 255],
                    scroll_rows,
                    thumb: Some(Box::new(ScrollThumb {
                        track: Rect::new(110., 12., 10., 60.),
                        clip: Rect::new(0., 0., 128., 96.),
                        images: [None, None, None],
                    })),
                }],
                ..Default::default()
            };
            let frame = make_frame(0, 96.);
            renderer.set_ui_scaled(&frame, scale);
            let top = renderer.ui_text_scroll_metrics()[0].clone();
            assert_eq!(top.first_row, 0);
            assert!(top.total_rows > top.visible_rows);
            let pixels = draw_scaled(&mut renderer, &frame, scale);
            let stride = (128. * scale) as usize;
            let pixel = |x: usize, y: usize| {
                &pixels[((y as f32 * scale) as usize * stride + (x as f32 * scale) as usize) * 4..]
                    [..3]
            };
            assert_eq!(pixel(115, 14), &[153, 139, 100]);
            assert_eq!(pixel(115, 70), &[0, 0, 0]);
            for (index, pixel) in pixels.chunks_exact(4).enumerate() {
                if pixel[0] > 0 && pixel[1] == 0 {
                    let x = (index % stride) as f32 / scale;
                    let y = (index / stride) as f32 / scale;
                    assert!((10.0..100.).contains(&x) && (12.0..40.).contains(&y));
                }
            }
            let frame = make_frame(usize::MAX, 96.);
            renderer.set_ui_scaled(&frame, scale);
            let bottom = renderer.ui_text_scroll_metrics()[0].clone();
            assert_eq!(bottom.first_row, bottom.max_scroll());
            assert_eq!(bottom.total_rows, top.total_rows);
            let pixels = draw_scaled(&mut renderer, &frame, scale);
            let pixel = |x: usize, y: usize| {
                &pixels[((y as f32 * scale) as usize * stride + (x as f32 * scale) as usize) * 4..]
                    [..3]
            };
            assert_eq!(pixel(115, 14), &[0, 0, 0]);
            assert_eq!(pixel(115, 70), &[153, 139, 100]);
            renderer.set_ui_scaled(&make_frame(usize::MAX, 36.), scale);
            assert!(renderer.ui_text_scroll_metrics()[0].total_rows > top.total_rows);

            let ui = renderer.ui.as_ref().unwrap();
            let px = (12. * scale) as u32;
            let long = "é雪".repeat(40);
            let ranges = ui.text_row_ranges(&long, px, 30.);
            assert!(ranges.len() > 10);
            assert_eq!(
                ranges
                    .iter()
                    .map(|range| &long[range.clone()])
                    .collect::<String>(),
                long
            );
            assert_eq!(ui.text_row_ranges("A\n\nB", px, 100.).len(), 3);
        }
    }

    fn draw(renderer: &mut crate::Renderer, frame: &UiFrame) -> Vec<u8> {
        draw_scaled(renderer, frame, 1.)
    }

    fn draw_scaled(renderer: &mut crate::Renderer, frame: &UiFrame, scale: f32) -> Vec<u8> {
        let mut ui = UiRenderer::new(&renderer.device, &renderer.queue, renderer.config.format);
        ui.prepare_scaled(
            &renderer.device,
            &renderer.queue,
            frame,
            [renderer.width, renderer.height],
            scale,
        );
        let mut encoder = renderer.device.create_command_encoder(&Default::default());
        let crate::Target::Offscreen { view, .. } = &renderer.target else {
            panic!("headless target required")
        };
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("UI test"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            ui.render(&mut pass);
        }
        renderer.queue.submit(Some(encoder.finish()));
        renderer.read_rgba().unwrap().2
    }

    #[test]
    fn clips_alpha_blended_quads_and_renders_text_on_gpu() {
        let Ok(mut renderer) = crate::Renderer::new_headless(128, 64) else {
            return;
        };
        let frame = UiFrame {
            commands: vec![
                DrawCommand::Fill {
                    rect: Rect::new(0., 0., 64., 64.),
                    clip: Rect::new(4., 8., 20., 16.),
                    color: [255, 0, 0, 255],
                },
                DrawCommand::Fill {
                    rect: Rect::new(8., 8., 8., 8.),
                    clip: Rect::new(0., 0., 64., 64.),
                    color: [0, 255, 0, 128],
                },
                DrawCommand::Text {
                    rect: Rect::new(68., 0., 60., 40.),
                    clip: Rect::new(68., 0., 60., 40.),
                    text: "OpenEQ".into(),
                    font: 3,
                    color: [255; 4],
                    align: TextAlign::Left,
                    vertical_center: false,
                    wrap: false,
                },
            ],
            ..Default::default()
        };
        let pixels = draw(&mut renderer, &frame);
        let pixel = |x: usize, y: usize| &pixels[(y * 128 + x) * 4..(y * 128 + x) * 4 + 3];
        assert_eq!(pixel(3, 10), &[0, 0, 0]);
        assert_eq!(pixel(4, 10), &[255, 0, 0]);
        assert_eq!(pixel(24, 10), &[0, 0, 0]);
        let blended = pixel(10, 10);
        assert!(
            (180..=195).contains(&blended[0])
                && (180..=195).contains(&blended[1])
                && blended[2] == 0,
            "linear alpha blending on sRGB target: {blended:?}"
        );
        assert!(
            pixels
                .chunks_exact(4)
                .enumerate()
                .filter(|(i, p)| i % 128 >= 68 && p[0] > 20)
                .count()
                > 50,
            "text should produce visible glyphs"
        );
    }

    #[test]
    fn chat_links_follow_utf8_wrap_scroll_and_retina_coordinates() {
        let Ok(renderer) = crate::Renderer::new_headless(256, 256) else {
            return;
        };
        let mut ui = UiRenderer::new(&renderer.device, &renderer.queue, renderer.config.format);
        ui.font = None; // deterministic 9-logical-pixel bitmap advances
        let message = "Start café narrow passage end";
        let begin = message.find("café").unwrap();
        let end = message.find(" end").unwrap();
        let make_frame = |scroll_rows| UiFrame {
            commands: vec![DrawCommand::TextLog {
                rect: Rect::new(8., 8., 63., 90.),
                clip: Rect::new(8., 8., 63., 90.),
                font: 2,
                scroll_rows,
                lines: vec![
                    openeq_ui::TextLine {
                        text: "Older message ".repeat(20),
                        color: [255; 4],
                        links: vec![],
                    },
                    openeq_ui::TextLine {
                        text: message.into(),
                        color: [255; 4],
                        links: vec![
                            openeq_ui::TextLink {
                                range: begin..end,
                                id: 42,
                            },
                            openeq_ui::TextLink {
                                range: begin + 4..message.len() + 50,
                                id: 99,
                            },
                        ],
                    },
                ],
            }],
            ..Default::default()
        };
        for scale in [1., 2.] {
            ui.prepare_scaled(
                &renderer.device,
                &renderer.queue,
                &make_frame(0),
                [256, 256],
                scale,
            );
            assert!(
                ui.link_hits().len() >= 3,
                "long link must produce multiple wrapped regions"
            );
            for hit in ui.link_hits() {
                assert_eq!(hit.item, "chat:link:42");
                assert!(
                    hit.rect.x >= 8.
                        && hit.rect.right() <= 71.
                        && hit.rect.y >= 8.
                        && hit.rect.bottom() <= 98.
                );
            }
            let first = ui.link_hits()[0].rect;
            assert_eq!(
                first.width, 36.,
                "café has four glyphs, despite its five UTF8 bytes"
            );
        }
        ui.prepare_scaled(
            &renderer.device,
            &renderer.queue,
            &make_frame(usize::MAX),
            [256, 256],
            2.,
        );
        assert!(
            ui.link_hits().is_empty(),
            "scrolled-out links cannot be clicked"
        );
        ui.prepare_scaled(
            &renderer.device,
            &renderer.queue,
            &make_frame(0),
            [0, 0],
            2.,
        );
        assert!(
            ui.link_hits().is_empty(),
            "zero-size preparation clears stale regions"
        );
    }

    #[test]
    fn retina_scales_geometry_scissors_and_font_rasterization() {
        let Ok(mut renderer) = crate::Renderer::new_headless(128, 64) else {
            return;
        };
        let frame = UiFrame {
            commands: vec![
                DrawCommand::Fill {
                    rect: Rect::new(4., 8., 20., 16.),
                    clip: Rect::new(6., 10., 8., 8.),
                    color: [255, 0, 0, 255],
                },
                DrawCommand::Text {
                    rect: Rect::new(36., 2., 28., 25.),
                    clip: Rect::new(36., 2., 28., 25.),
                    text: "Hi".into(),
                    font: 2,
                    color: [0, 255, 0, 255],
                    align: TextAlign::Left,
                    vertical_center: false,
                    wrap: false,
                },
            ],
            ..Default::default()
        };
        let pixels = draw_scaled(&mut renderer, &frame, 2.);
        let pixel = |x: usize, y: usize| &pixels[(y * 128 + x) * 4..(y * 128 + x) * 4 + 3];
        assert_eq!(pixel(12, 20), &[255, 0, 0]);
        assert_eq!(pixel(27, 35), &[255, 0, 0]);
        assert_eq!(pixel(11, 20), &[0, 0, 0]);
        assert_eq!(pixel(28, 20), &[0, 0, 0]);
        assert_eq!(pixel(12, 36), &[0, 0, 0]);
        assert!(
            pixels
                .chunks_exact(4)
                .enumerate()
                .filter(|(i, p)| i % 128 >= 72 && p[1] > 20)
                .count()
                > 100
        );
        let mut ui = UiRenderer::new(&renderer.device, &renderer.queue, renderer.config.format);
        ui.prepare_scaled(&renderer.device, &renderer.queue, &frame, [128, 64], 2.);
        assert!(
            ui.glyphs.contains_key(&('H', 24)),
            "12px logical font is rasterized at 24 physical pixels"
        );
        assert!(!ui.glyphs.contains_key(&('H', 12)));
    }

    #[test]
    fn diagonal_map_lines_keep_thickness_and_clip() {
        let Ok(mut renderer) = crate::Renderer::new_headless(64, 64) else {
            return;
        };
        let frame = UiFrame {
            commands: vec![
                DrawCommand::Line {
                    from: [0., 0.],
                    to: [64., 64.],
                    width: 3.,
                    clip: Rect::new(8., 8., 40., 40.),
                    color: [255, 0, 0, 255],
                },
                DrawCommand::Line {
                    from: [f32::NAN, 0.],
                    to: [64., 64.],
                    width: 3.,
                    clip: Rect::new(0., 0., 64., 64.),
                    color: [0, 255, 0, 255],
                },
            ],
            ..Default::default()
        };
        let pixels = draw(&mut renderer, &frame);
        let pixel = |x: usize, y: usize| &pixels[(y * 64 + x) * 4..(y * 64 + x) * 4 + 3];
        assert_eq!(pixel(20, 20), &[255, 0, 0]);
        assert_eq!(pixel(20, 28), &[0, 0, 0]);
        assert_eq!(pixel(4, 4), &[0, 0, 0]);
        assert_eq!(pixel(52, 52), &[0, 0, 0]);
    }

    #[test]
    fn chat_log_wraps_colors_and_scrolls_to_oldest_without_leaking_clip() {
        let Ok(mut renderer) = crate::Renderer::new_headless(128, 64) else {
            return;
        };
        let make_frame = |scroll_rows| {
            UiFrame {
            commands: vec![DrawCommand::TextLog {
                rect: Rect::new(8., 8., 90., 34.),
                clip: Rect::new(8., 8., 90., 34.),
                lines: vec![
                    openeq_ui::TextLine { text: "An old red message that wraps across several displayed rows in the chat window".into(), color: [255, 0, 0, 255], links: vec![] },
                    openeq_ui::TextLine { text: "Newest".into(), color: [0, 255, 0, 255], links: vec![] },
                ],
                font: 2,
                scroll_rows,
            }],
            ..Default::default()
        }
        };
        let latest = draw(&mut renderer, &make_frame(0));
        let oldest = draw(&mut renderer, &make_frame(usize::MAX));
        let channel_pixels = |pixels: &[u8], channel: usize| {
            pixels.chunks_exact(4).filter(|p| p[channel] > 30).count()
        };
        assert!(
            channel_pixels(&latest, 0) > 10,
            "previous wrapped row remains visible"
        );
        assert!(
            channel_pixels(&latest, 1) > 10,
            "latest row keeps its green color"
        );
        assert!(
            channel_pixels(&oldest, 0) > 10,
            "large scroll offsets clamp to oldest page"
        );
        assert_eq!(
            channel_pixels(&oldest, 1),
            0,
            "newest row scrolls out of view"
        );
        for (i, p) in latest.chunks_exact(4).enumerate() {
            if i % 128 < 8 || i % 128 >= 98 || i / 128 < 8 || i / 128 >= 42 {
                assert_eq!(&p[..3], &[0, 0, 0], "chat escaped its clip");
            }
        }
    }

    #[test]
    #[ignore = "requires original client UI assets and GPU; optionally set OPENEQ_UI_CAPTURE_DIR"]
    fn renders_actual_login_and_player_xml() {
        let directory = std::env::var_os("EQ_UI_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").unwrap()).join("EverQuest/uifiles/default")
            });
        for (entry, window, size, filename) in [
            ("EQLSUI.xml", "connect", [640, 480], "openeq-xml-login.png"),
            (
                "EQUI.xml",
                "PlayerWindow",
                [320, 160],
                "openeq-xml-player.png",
            ),
        ] {
            let document = openeq_ui::UiDocument::load(&directory, entry).unwrap();
            let mut bindings = openeq_ui::UiBindings::default();
            bindings.widget_mut("UsernameEdit").text = Some("OpenEQ".into());
            bindings.widget_mut("Player_HP").text = Some("Adventurer".into());
            bindings
                .eq_gauges
                .extend([("1".into(), 0.73), ("2".into(), 0.85), ("3".into(), 1.)]);
            bindings.eq_text.extend([
                ("19".into(), "73".into()),
                ("20".into(), "85".into()),
                ("21".into(), "100".into()),
            ]);
            let frame = document
                .window(window)
                .unwrap()
                .layout(Rect::new(0., 0., size[0] as f32, size[1] as f32), &bindings);
            let mut renderer = crate::Renderer::new_headless(size[0], size[1]).unwrap();
            let pixels = draw(&mut renderer, &frame);
            assert!(
                pixels
                    .chunks_exact(4)
                    .filter(|p| p[0] > 10 || p[1] > 10 || p[2] > 10)
                    .count()
                    > 2000
            );
            if let Some(destination) = std::env::var_os("OPENEQ_UI_CAPTURE_DIR") {
                let path = PathBuf::from(destination).join(filename);
                image::save_buffer(&path, &pixels, size[0], size[1], image::ColorType::Rgba8)
                    .unwrap();
                eprintln!("wrote {}", path.display());
            }
        }
    }
}
