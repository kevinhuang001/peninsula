use crate::ui::declarative::RasterImage;
use std::{
    io::Cursor,
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) const MAX_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Default)]
pub(super) struct Cache {
    key: String,
    result: Option<Arc<RasterImage>>,
    retry_at: Option<Instant>,
}

impl Cache {
    pub fn get(
        &mut self,
        key: &str,
        load: impl FnOnce() -> Option<Vec<u8>>,
    ) -> Option<Arc<RasterImage>> {
        let now = Instant::now();
        if self.key != key
            || (self.result.is_none() && self.retry_at.is_none_or(|retry| now >= retry))
        {
            self.result = load().and_then(|data| decode(&data)).map(Arc::new);
            self.key = key.to_owned();
            self.retry_at = Some(now + Duration::from_secs(10));
        }
        self.result.clone()
    }
}

fn decode(data: &[u8]) -> Option<RasterImage> {
    let mut reader = image::ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(32 * 1024 * 1024);
    reader.limits(limits);
    let mut image = reader.decode().ok()?;
    // Crop to the artwork slot at twice its display size.
    const WIDTH: u32 = 92;
    const HEIGHT: u32 = 136;
    image = image.resize_to_fill(WIDTH, HEIGHT, image::imageops::FilterType::Triangle);
    Some(RasterImage {
        size: [WIDTH as u16, HEIGHT as u16],
        rgba: image.into_rgba8().into_raw(),
    })
}

#[cfg(target_os = "linux")]
pub(super) fn read_url(raw: &str) -> Option<Vec<u8>> {
    use std::{fs::File, io::Read};
    let url = url::Url::parse(raw).ok()?;
    let input: Box<dyn Read> = match url.scheme() {
        "file" => {
            let path = url.to_file_path().ok()?;
            let metadata = std::fs::metadata(&path).ok()?;
            if !metadata.is_file() || metadata.len() > MAX_BYTES {
                return None;
            }
            Box::new(File::open(path).ok()?)
        }
        "http" | "https" => {
            let response = ureq::get(url.as_str())
                .timeout(Duration::from_secs(2))
                .call()
                .ok()?;
            response.into_reader()
        }
        _ => return None,
    };
    let mut bytes = Vec::new();
    input.take(MAX_BYTES + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= MAX_BYTES).then_some(bytes)
}
