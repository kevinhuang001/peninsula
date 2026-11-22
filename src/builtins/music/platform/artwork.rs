use crate::ui::declarative::RasterImage;
use std::{
    io::Cursor,
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) const MAX_BYTES: u64 = 2 * 1024 * 1024;

type Artwork = Option<Arc<RasterImage>>;

#[derive(Default)]
pub(super) struct Cache {
    key: String,
    result: Artwork,
    retry_at: Option<Instant>,
    pending: Option<std::sync::mpsc::Receiver<Artwork>>,
}

impl Cache {
    pub fn get(
        &mut self,
        tasks: &crate::sdk::tasks::TaskScope,
        key: &str,
        load: impl FnOnce(&crate::sdk::tasks::CancellationToken) -> Option<Vec<u8>> + Send + 'static,
    ) -> Artwork {
        if self.key != key {
            *self = Self {
                key: key.into(),
                ..Default::default()
            };
        }
        if let Some(result) = self.pending.as_ref().map(|pending| pending.try_recv()) {
            match result {
                Ok(image) => {
                    self.result = image;
                    self.pending = None;
                    self.retry_at = Some(Instant::now() + Duration::from_secs(10));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.pending = None;
                    self.retry_at = Some(Instant::now() + Duration::from_secs(10));
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.result.is_none()
            && self.pending.is_none()
            && self.retry_at.is_none_or(|retry| Instant::now() >= retry)
        {
            let (sender, receiver) = std::sync::mpsc::channel();
            if tasks
                .spawn("music-artwork", move |cancel| {
                    if !cancel.is_cancelled() {
                        let image = load(&cancel).and_then(|bytes| decode(&bytes)).map(Arc::new);
                        if !cancel.is_cancelled() {
                            let _ = sender.send(image);
                        }
                    }
                })
                .is_ok()
            {
                self.pending = Some(receiver);
            } else {
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
pub(super) fn read_url(
    raw: &str,
    cancellation: &crate::sdk::tasks::CancellationToken,
) -> Option<Vec<u8>> {
    use std::{fs::File, io::Read};
    let url = url::Url::parse(raw).ok()?;
    match url.scheme() {
        "file" => {
            let path = url.to_file_path().ok()?;
            let metadata = std::fs::metadata(&path).ok()?;
            if !metadata.is_file() || metadata.len() > MAX_BYTES {
                return None;
            }
            let mut bytes = Vec::new();
            File::open(path)
                .ok()?
                .take(MAX_BYTES + 1)
                .read_to_end(&mut bytes)
                .ok()?;
            (bytes.len() as u64 <= MAX_BYTES).then_some(bytes)
        }
        "http" | "https" => crate::sdk::http::run_cancellable(cancellation, async {
            let timeout = Duration::from_secs(2);
            let client = reqwest::Client::builder()
                .connect_timeout(timeout)
                .timeout(timeout)
                .user_agent(concat!("Peninsula/", env!("CARGO_PKG_VERSION")))
                .build()
                .ok()?;
            let mut response = client.get(url).send().await.ok()?.error_for_status().ok()?;
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.ok()? {
                if bytes.len().saturating_add(chunk.len()) as u64 > MAX_BYTES {
                    return None;
                }
                bytes.extend_from_slice(&chunk);
            }
            Some(bytes)
        })
        .ok()
        .flatten()
        .flatten(),
        _ => None,
    }
}

#[cfg(test)]
mod freshness_tests {
    use super::*;
    #[test]
    fn slow_artwork_loader_never_blocks_media_observation() {
        let mut cache = Cache::default();
        let tasks = crate::sdk::tasks::TaskScope::default();
        let (release, wait) = std::sync::mpsc::channel();
        let start = Instant::now();
        assert!(cache
            .get(&tasks, "slow", move |_| {
                let _ = wait.recv_timeout(Duration::from_secs(2));
                None
            })
            .is_none());
        assert!(start.elapsed() < Duration::from_millis(200));
        release.send(()).unwrap();
        assert!(tasks.shutdown());
    }
}
