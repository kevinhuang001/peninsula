use crate::ui::declarative::RasterImage;
use std::{
    io::Cursor,
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) const MAX_BYTES: u64 = 2 * 1024 * 1024;

type Load = Box<dyn FnOnce() -> Option<Vec<u8>> + Send>;
type Artwork = Option<Arc<RasterImage>>;

#[derive(Default)]
pub(super) struct Cache {
    key: String,
    result: Artwork,
    retry_at: Option<Instant>,
    loading: bool,
    worker: Option<(
        std::sync::mpsc::Sender<(String, Load)>,
        std::sync::mpsc::Receiver<(String, Artwork)>,
    )>,
}

impl Cache {
    pub fn get(
        &mut self,
        key: &str,
        load: impl FnOnce() -> Option<Vec<u8>> + Send + 'static,
    ) -> Artwork {
        if self.worker.is_none() {
            let (tx, requests) = std::sync::mpsc::channel::<(String, Load)>();
            let (results, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                while let Ok(first) = requests.recv() {
                    let (key, load) = requests.try_iter().last().unwrap_or(first);
                    let image = load().and_then(|bytes| decode(&bytes)).map(Arc::new);
                    if results.send((key, image)).is_err() {
                        break;
                    }
                }
            });
            self.worker = Some((tx, rx));
        }
        if self.key != key {
            self.key = key.into();
            self.result = None;
            self.retry_at = None;
            self.loading = false;
        }
        let (tx, rx) = self.worker.as_ref().unwrap();
        while let Ok((finished, image)) = rx.try_recv() {
            if finished == self.key {
                self.result = image;
                self.loading = false;
                self.retry_at = Some(Instant::now() + Duration::from_secs(10));
            }
        }
        if self.result.is_none()
            && !self.loading
            && self.retry_at.is_none_or(|retry| Instant::now() >= retry)
        {
            self.loading = tx.send((key.into(), Box::new(load))).is_ok();
            if !self.loading {
                self.retry_at = Some(Instant::now() + Duration::from_secs(10));
            }
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
            let timeout = Duration::from_secs(2);
            let client = crate::core::http::client(
                timeout,
                timeout,
                concat!("Peninsula/", env!("CARGO_PKG_VERSION")),
            )
            .ok()?;
            Box::new(client.get(url).send().ok()?.error_for_status().ok()?)
        }
        _ => return None,
    };
    let mut bytes = Vec::new();
    input.take(MAX_BYTES + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= MAX_BYTES).then_some(bytes)
}

#[cfg(test)]
mod freshness_tests {
    use super::*;
    #[test]
    fn slow_artwork_loader_never_blocks_media_observation() {
        let mut cache = Cache::default();
        let (release, wait) = std::sync::mpsc::channel();
        let start = Instant::now();
        assert!(cache
            .get("slow", move || {
                let _ = wait.recv_timeout(Duration::from_secs(2));
                None
            })
            .is_none());
        assert!(start.elapsed() < Duration::from_millis(200));
        release.send(()).unwrap();
    }
}
