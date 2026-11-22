use super::Theme;
use crate::ui::declarative::{ImageFit, SvgStyle};
use eframe::egui::{self, pos2, Color32, Id, Rect, TextureHandle, Ui, Vec2};

pub(super) fn image_id(asset: &str) -> Id {
    Id::new(("widget-image", asset))
}

pub(super) fn image(ui: &Ui, asset: &str) -> Option<TextureHandle> {
    if let Some(texture) = ui.data(|data| data.get_temp(image_id(asset))) {
        return Some(texture);
    }
    let bytes = ui.data(|data| data.get_temp::<&'static [u8]>(Id::new(("widget-png", asset))))?;
    let decoded = ::image::load_from_memory_with_format(bytes, ::image::ImageFormat::Png)
        .ok()?
        .into_rgba8();
    let pixels = egui::ColorImage::from_rgba_unmultiplied(
        [decoded.width() as usize, decoded.height() as usize],
        decoded.as_raw(),
    );
    let texture = ui
        .ctx()
        .load_texture(asset, pixels, egui::TextureOptions::LINEAR);
    super::register_image(ui.ctx(), asset, texture.clone());
    Some(texture)
}

pub(crate) fn register_png(ctx: &egui::Context, asset: &str, bytes: &'static [u8]) {
    ctx.data_mut(|data| data.insert_temp(Id::new(("widget-png", asset)), bytes));
}

/// Resource registration is host-side. View descriptors reference catalog keys.
pub(crate) fn register_svg(ctx: &egui::Context, asset: &str, source: &'static str) {
    ctx.data_mut(|data| data.insert_temp(Id::new(("widget-svg", asset)), source));
}

pub(super) fn svg(
    ui: &Ui,
    theme: &Theme,
    asset: &str,
    style: SvgStyle,
    size: Option<Vec2>,
) -> Option<TextureHandle> {
    let source = asset
        .strip_prefix("inline:")
        .map(str::to_owned)
        .or_else(|| {
            ui.data(|data| data.get_temp::<&'static str>(Id::new(("widget-svg", asset))))
                .map(str::to_owned)
        })?;
    let fill = style.fill.map(|color| theme.color(ui, color));
    let stroke = style.stroke.map(|color| theme.color(ui, color));
    let width = style
        .stroke_width
        .filter(|width| width.is_finite())
        .map(|width| (width.clamp(0.0, 32.0) * 16.0).round() as u16);
    let extent = size
        .map(|size| {
            let scale = ui.ctx().pixels_per_point();
            [
                (size.x * scale).ceil().clamp(1.0, 512.0) as u32,
                (size.y * scale).ceil().clamp(1.0, 512.0) as u32,
            ]
        })
        .unwrap_or([128, 128]);
    let key = Id::new((asset, fill, stroke, width, extent));
    let svg_id = Id::new("widget-svg-textures");
    if let Some(texture) = ui.data_mut(|data| {
        data.get_temp_mut_or_default::<Vec<(Id, TextureHandle)>>(svg_id)
            .iter()
            .find(|(id, _)| *id == key)
            .map(|(_, texture)| texture.clone())
    }) {
        return Some(texture);
    }

    // Host SVGs opt into styling through the fill and stroke attributes.
    let mut source = source.replace("currentColor", "#ffffff");
    let css_color = |color: Color32| {
        let [r, g, b, _] = color.to_srgba_unmultiplied();
        format!("#{r:02x}{g:02x}{b:02x}")
    };
    for (attribute, value) in [
        ("fill", fill.map(css_color)),
        ("stroke", stroke.map(css_color)),
        (
            "stroke-width",
            width.map(|value| (f32::from(value) / 16.0).to_string()),
        ),
    ] {
        if let Some(value) = value {
            source = root_attribute(&source, attribute, &value);
        }
    }
    for (attribute, color) in [("fill-opacity", fill), ("stroke-opacity", stroke)] {
        if let Some(color) = color {
            source = root_attribute(
                &source,
                attribute,
                &(f32::from(color.a()) / 255.0).to_string(),
            );
        }
    }
    // Render at 4x the target physical size, then area-filter down in
    // premultiplied alpha. A fixed 128px texture sampled at tiny icon sizes
    // aliases thin circles and hands, particularly at fractional DPI scales.
    let pixels = egui_extras::image::load_svg_bytes_with_size(
        source.as_bytes(),
        Some(egui::SizeHint::Size(extent[0] * 4, extent[1] * 4)),
    )
    .ok()?;
    let dimensions = [
        ((pixels.size[0] as u32 + 2) / 4).max(1),
        ((pixels.size[1] as u32 + 2) / 4).max(1),
    ];
    let rgba: Vec<u8> = pixels
        .pixels
        .iter()
        .flat_map(|pixel| pixel.to_array())
        .collect();
    let image = ::image::RgbaImage::from_raw(pixels.size[0] as u32, pixels.size[1] as u32, rgba)?;
    let filtered = ::image::imageops::resize(
        &image,
        dimensions[0],
        dimensions[1],
        ::image::imageops::FilterType::Triangle,
    );
    let pixels = egui::ColorImage {
        size: dimensions.map(|v| v as usize),
        pixels: filtered
            .pixels()
            .map(|pixel| Color32::from_rgba_premultiplied(pixel[0], pixel[1], pixel[2], pixel[3]))
            .collect(),
    };
    let texture = ui
        .ctx()
        .load_texture(asset, pixels, egui::TextureOptions::LINEAR);
    ui.data_mut(|data| {
        let cache = data.get_temp_mut_or_default::<Vec<(Id, TextureHandle)>>(svg_id);
        if cache.len() == 64 {
            drop(cache.remove(0));
        }
        cache.push((key, texture.clone()));
    });
    Some(texture)
}

fn root_attribute(source: &str, name: &str, value: &str) -> String {
    let Some(start) = source.find("<svg").map(|start| start + 4) else {
        return source.into();
    };
    let Some(end) = source[start..].find('>').map(|end| end + start) else {
        return source.into();
    };
    let root = &source[start..end];
    let attribute = format!("{name}=\"");
    if let Some(at) = root
        .match_indices(&attribute)
        .find_map(|(at, _)| (at > 0 && root.as_bytes()[at - 1].is_ascii_whitespace()).then_some(at))
    {
        let from = start + at + attribute.len();
        if let Some(to) = source[from..end].find('"').map(|to| from + to) {
            return format!("{}{value}{}", &source[..from], &source[to..]);
        }
    }
    format!(
        "{} {name}=\"{value}\"{}",
        &source[..start],
        &source[start..]
    )
}

pub(super) fn paint_texture(
    painter: &egui::Painter,
    texture: &TextureHandle,
    mut rect: Rect,
    mut uv: Rect,
    fit: ImageFit,
    radius: f32,
    tint: Color32,
) {
    let native = texture.size_vec2() * uv.size();
    if !rect.is_positive() || native.x <= 0.0 || native.y <= 0.0 {
        return;
    }
    let scale = rect.size() / native;
    match fit {
        ImageFit::Contain => {
            rect = Rect::from_center_size(rect.center(), native * scale.min_elem())
        }
        ImageFit::Cover => {
            uv = Rect::from_center_size(
                uv.center(),
                uv.size() * (scale / Vec2::splat(scale.max_elem())),
            )
        }
        ImageFit::Stretch => {}
    }
    painter.add(egui::epaint::RectShape::filled(rect, radius, tint).with_texture(texture.id(), uv));
}

pub(super) fn uv_rect(uv: [f32; 4]) -> Rect {
    let [left, top, right, bottom] = uv.map(|value| {
        if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.0
        }
    });
    Rect::from_min_max(pos2(left, top), pos2(right, bottom))
}

pub(super) fn svg_tint(style: SvgStyle, foreground: Color32, opacity: f32) -> Color32 {
    if style.fill.is_some() || style.stroke.is_some() {
        Color32::WHITE.linear_multiply(opacity)
    } else {
        foreground
    }
}
