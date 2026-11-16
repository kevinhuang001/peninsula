use eframe::egui::{self, Color32, Rect, TextureHandle, TextureOptions};

const SHEETS: [&[u8]; 12] = [
    include_bytes!("../../assets/mascot/expressions/00-wink.png"),
    include_bytes!("../../assets/mascot/expressions/01-smug.png"),
    include_bytes!("../../assets/mascot/expressions/02-blush.png"),
    include_bytes!("../../assets/mascot/expressions/03-delighted.png"),
    include_bytes!("../../assets/mascot/expressions/04-side-glance.png"),
    include_bytes!("../../assets/mascot/expressions/05-praised.png"),
    include_bytes!("../../assets/mascot/expressions/06-heart.png"),
    include_bytes!("../../assets/mascot/expressions/07-giggle.png"),
    include_bytes!("../../assets/mascot/expressions/08-shy-peek.png"),
    include_bytes!("../../assets/mascot/expressions/09-scan.png"),
    include_bytes!("../../assets/mascot/expressions/10-peek.png"),
    include_bytes!("../../assets/mascot/expressions/11-search.png"),
];

const EXPRESSION_TIMES: [f32; 16] = [
    0.35, 0.12, 0.12, 0.12, 0.13, 0.13, 0.14, 0.40, 0.18, 0.14, 0.12, 0.12, 0.12, 0.14, 0.14, 0.45,
];
const IDLE_TIMES: [f32; 16] = [
    0.35, 0.13, 0.15, 0.18, 0.35, 0.28, 0.16, 0.11, 0.11, 0.16, 0.35, 0.28, 0.16, 0.14, 0.20, 0.35,
];

pub(super) struct Mascot {
    expression: usize,
    elapsed: f32,
    idle_wait: f32,
    playing: bool,
    texture: Option<(usize, TextureHandle)>,
}

impl Default for Mascot {
    fn default() -> Self {
        Self {
            expression: 0,
            elapsed: 0.0,
            idle_wait: 4.0,
            playing: false,
            texture: None,
        }
    }
}

impl Mascot {
    pub fn advance(&mut self, dt: f32, clicked: bool) {
        if clicked {
            self.expression = (self.expression + 1) % SHEETS.len();
            self.elapsed = 0.0;
            self.playing = true;
            self.idle_wait = 7.0;
        } else if self.playing {
            self.elapsed += dt;
            if self.elapsed >= duration(self.expression) {
                self.playing = false;
                self.elapsed = 0.0;
                self.idle_wait = 7.0;
            }
        } else {
            self.idle_wait -= dt;
            if self.idle_wait <= 0.0 {
                self.expression = (self.expression + 1) % SHEETS.len();
                self.elapsed = 0.0;
                self.playing = true;
            }
        }
    }

    #[cfg(test)]
    pub fn animating(&self) -> bool {
        self.playing
    }

    pub fn paint(
        &mut self,
        context: &egui::Context,
        painter: &egui::Painter,
        rect: Rect,
        opacity: f32,
    ) {
        if self
            .texture
            .as_ref()
            .is_none_or(|(index, _)| *index != self.expression)
        {
            let decoded = image::load_from_memory(SHEETS[self.expression])
                .expect("bundled mascot sheet")
                .into_rgba8();
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [decoded.width() as usize, decoded.height() as usize],
                decoded.as_raw(),
            );
            let texture = context.load_texture("island-mascot", image, TextureOptions::LINEAR);
            self.texture = Some((self.expression, texture));
        }
        let frame = frame_at(self.expression, self.elapsed, self.playing);
        let uv = Rect::from_min_max(
            egui::pos2(0.0, frame as f32 / 16.0),
            egui::pos2(1.0, (frame + 1) as f32 / 16.0),
        );
        if let Some((_, texture)) = &self.texture {
            painter.image(
                texture.id(),
                rect,
                uv,
                Color32::WHITE.linear_multiply(opacity.clamp(0.0, 1.0)),
            );
        }
    }
}

fn frame_at(expression: usize, mut elapsed: f32, playing: bool) -> usize {
    if !playing {
        return 0;
    }
    let times = if expression >= 9 {
        IDLE_TIMES
    } else {
        EXPRESSION_TIMES
    };
    for (index, frame_time) in times.into_iter().enumerate() {
        if elapsed < frame_time {
            return index;
        }
        elapsed -= frame_time;
    }
    15
}

fn duration(expression: usize) -> f32 {
    if expression >= 9 {
        IDLE_TIMES.iter().sum()
    } else {
        EXPRESSION_TIMES.iter().sum()
    }
}

#[cfg(test)]
mod tests {
    use super::{duration, frame_at, Mascot, SHEETS};
    use image::GenericImageView;

    #[test]
    fn mascot_starts_with_an_open_expression_and_idle_animation() {
        let mascot = Mascot::default();
        assert!(!mascot.animating());
        assert_eq!(frame_at(0, 0.0, false), 0);
    }

    #[test]
    fn idle_expression_plays_a_full_timed_sprite_sequence() {
        let mut mascot = Mascot::default();
        mascot.advance(4.1, false);
        assert!(mascot.animating());
        assert_eq!(frame_at(mascot.expression, 0.0, true), 0);
        assert!(frame_at(mascot.expression, duration(mascot.expression) + 1.0, true) >= 15);
        mascot.advance(duration(mascot.expression), false);
        assert!(!mascot.animating());
    }

    #[test]
    fn click_starts_the_next_expression_immediately() {
        let mut mascot = Mascot::default();
        mascot.advance(0.0, true);
        assert!(mascot.animating());
        assert_eq!(mascot.expression, 1);
        mascot.advance(0.2, false);
        assert!(mascot.animating());
    }

    #[test]
    fn bundled_reference_art_has_sixteen_transparent_animation_frames() {
        for bytes in SHEETS {
            let image = image::load_from_memory(bytes).expect("valid expression PNG");
            assert_eq!(image.dimensions(), (144, 1440));
            assert!(image.color().has_alpha());
        }
    }
}
