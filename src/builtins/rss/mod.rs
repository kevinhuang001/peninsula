mod feed;
pub(crate) mod reader;
mod view;

use crate::extension::{
    Action, ContributionDescriptor, ContributionKind, ExtensionContext, ExtensionDescriptor,
    ExtensionId, ExtensionOutput, Notification, NotificationAction, NotificationKey,
    NotificationPresentationProvider, NotificationPriority, ReloadPolicy, RuntimeExtension,
    SettingSchema, SettingValue, Settings, WidgetView,
};
use crate::sdk::{storage::StorageArea, tasks::TaskScope};
use feed::{Article, Feed};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    hash::{DefaultHasher, Hash, Hasher},
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant, SystemTime},
};

const ID: &str = "builtin.rss";
const WIDGET_ID: &str = "builtin.rss/reader";
const NOTIFICATION_ID: &str = "builtin.rss/updates";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ReaderTarget {
    url: String,
    source: String,
    article: Article,
}

fn notification_id(url: &str, article_id: &str) -> String {
    let mut hash = DefaultHasher::new();
    (url, article_id).hash(&mut hash);
    format!("rss-update-{:016x}", hash.finish())
}

struct Fetcher {
    requests: Sender<Option<String>>,
    results: Receiver<(String, Result<Feed, String>)>,
}

impl Fetcher {
    fn start(tasks: TaskScope) -> Result<Self, String> {
        let (requests, input) = mpsc::channel::<Option<String>>();
        let (output, results) = mpsc::channel();
        tasks
            .spawn("rss-fetch", move |cancelled| {
                while let Ok(Some(url)) = input.recv() {
                    if cancelled.is_cancelled() {
                        break;
                    }
                    let result = feed::fetch(&url);
                    if cancelled.is_cancelled() || output.send((url, result)).is_err() {
                        break;
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self { requests, results })
    }
}

impl Drop for Fetcher {
    fn drop(&mut self) {
        let _ = self.requests.send(None);
    }
}

pub struct RssExtension {
    feeds: Vec<Feed>,
    selected: Option<String>,
    storage: Option<StorageArea>,
    fetcher: Option<Fetcher>,
    pending: HashSet<String>,
    errors: HashMap<String, String>,
    error: Option<String>,
    adding: bool,
    draft: String,
    refresh_interval: Duration,
    next_refresh: Instant,
    reader: Option<reader::Reader>,
}

impl Default for RssExtension {
    fn default() -> Self {
        Self {
            feeds: Vec::new(),
            selected: None,
            storage: None,
            fetcher: None,
            pending: HashSet::new(),
            errors: HashMap::new(),
            error: None,
            adding: false,
            draft: String::new(),
            refresh_interval: Duration::from_secs(600),
            next_refresh: Instant::now(),
            reader: None,
        }
    }
}

impl NotificationPresentationProvider for RssExtension {
    fn handles_notification(&self, source_id: &ExtensionId) -> bool {
        source_id.as_str() == NOTIFICATION_ID
    }

    fn on_notification_action(
        &mut self,
        notification: &Notification,
        action: &NotificationAction,
    ) -> Result<(), String> {
        if !self.handles_notification(&notification.source_id) || action.id != "view" {
            return Err("未知的 RSS 通知操作".into());
        }
        let target: ReaderTarget =
            serde_json::from_str(action.data.as_deref().ok_or("通知缺少文章信息")?)
                .map_err(|error| error.to_string())?;
        let result = self.open_article(&target);
        self.error = result.as_ref().err().cloned();
        result
    }
}

impl RssExtension {
    fn target(&self, url: &str, article_id: &str) -> Option<ReaderTarget> {
        let feed = self.feeds.iter().find(|feed| feed.url == url)?;
        let article = feed
            .articles
            .iter()
            .find(|article| article.id == article_id)?;
        Some(ReaderTarget {
            url: url.into(),
            source: feed.title.clone(),
            article: article.clone(),
        })
    }

    fn mark_read(&mut self, url: &str, article_id: &str, read: bool) -> Result<(), String> {
        let Some(index) = self.feeds.iter().position(|feed| feed.url == url) else {
            return Ok(());
        };
        let previous = self.feeds[index].read_ids.contains(article_id);
        if previous == read {
            return Ok(());
        }
        if read {
            self.feeds[index].read_ids.insert(article_id.into());
        } else {
            self.feeds[index].read_ids.remove(article_id);
        }
        if let Err(error) = self.save() {
            if previous {
                self.feeds[index].read_ids.insert(article_id.into());
            } else {
                self.feeds[index].read_ids.remove(article_id);
            }
            return Err(format!("阅读状态保存失败：{error}"));
        }
        Ok(())
    }

    fn open_article(&mut self, target: &ReaderTarget) -> Result<(), String> {
        // A closed native window may retain a detached WebKit layer tree.
        // Recreate it on the next open instead of reusing that drawing surface.
        if self.reader.as_ref().is_none_or(reader::Reader::is_closed) {
            self.reader = None;
            self.reader = Some(reader::Reader::new()?);
        }
        let current = self
            .target(&target.url, &target.article.id)
            .unwrap_or_else(|| target.clone());
        self.reader
            .as_ref()
            .unwrap()
            .show(&current.article, &current.source)?;
        if self.feeds.iter().any(|feed| feed.url == target.url) {
            self.selected = Some(target.url.clone());
        }
        self.mark_read(&target.url, &target.article.id, true)
    }

    fn receive_feed(
        &mut self,
        index: usize,
        incoming: Feed,
    ) -> Result<Option<Notification>, String> {
        let old = self.feeds[index].clone();
        let (merged, fresh) = old.merge(incoming);
        self.feeds[index] = merged;
        if let Err(error) = self.save() {
            self.feeds[index] = old;
            return Err(format!("文章缓存保存失败：{error}"));
        }
        let Some(article) = fresh.first() else {
            return Ok(None);
        };
        let feed = &self.feeds[index];
        let target = ReaderTarget {
            url: feed.url.clone(),
            source: feed.title.clone(),
            article: article.clone(),
        };
        Ok(Some(Notification {
            source_id: ExtensionId::builtin(NOTIFICATION_ID),
            id: notification_id(&feed.url, &article.id),
            title: format!("{} · {} 篇新文章", feed.title, fresh.len()),
            body: article.title.clone(),
            icon: Some("rss".into()),
            priority: NotificationPriority::Normal,
            actions: vec![NotificationAction {
                id: "view".into(),
                label: "查看".into(),
                data: Some(serde_json::to_string(&target).map_err(|error| error.to_string())?),
            }],
            created_at: SystemTime::now(),
        }))
    }

    fn changed(&self) -> ExtensionOutput {
        ExtensionOutput {
            invalidated: vec![ExtensionId::builtin(WIDGET_ID)],
            ..Default::default()
        }
    }

    fn save(&self) -> Result<(), String> {
        self.storage
            .as_ref()
            .ok_or("RSS 尚未启动")?
            .write_json("subscriptions.json", &self.feeds)
            .map_err(|error| error.to_string())
    }

    fn request(&mut self, url: String) {
        if self.pending.contains(&url) {
            return;
        }
        match self
            .fetcher
            .as_ref()
            .ok_or("RSS 尚未启动".to_owned())
            .and_then(|fetcher| {
                fetcher
                    .requests
                    .send(Some(url.clone()))
                    .map_err(|error| error.to_string())
            }) {
            Ok(()) => {
                self.pending.insert(url);
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn subscribe(&mut self) -> Result<(), String> {
        let url = feed::normalize_url(&self.draft)?;
        if !self.feeds.iter().any(|feed| feed.url == url) {
            let title = url::Url::parse(&url)
                .ok()
                .and_then(|url| url.host_str().map(str::to_owned))
                .unwrap_or_else(|| url.clone());
            self.feeds.push(Feed {
                url: url.clone(),
                title,
                articles: Vec::new(),
                initialized: false,
                seen_ids: Default::default(),
                read_ids: Default::default(),
            });
            if let Err(error) = self.save() {
                self.feeds.pop();
                return Err(format!("订阅保存失败：{error}"));
            }
        }
        self.selected = Some(url.clone());
        self.adding = false;
        self.draft.clear();
        self.request(url);
        Ok(())
    }
}

impl RuntimeExtension for RssExtension {
    fn descriptor(&self) -> ExtensionDescriptor {
        let widget = ContributionDescriptor::builtin(
            WIDGET_ID,
            ContributionKind::Widget,
            "RSS",
            "rss",
            true,
            8,
        )
        .with_size_hint(580.0, 340.0)
        .with_widget_bounds(crate::ui::declarative::WidgetBounds {
            min: [400.0, 240.0],
            max: [Some(760.0), Some(560.0)],
            resizable: true,
            fit_height: true,
            overflow: crate::ui::declarative::Overflow::Scroll(
                crate::ui::declarative::ScrollAxes::Vertical,
            ),
            ..Default::default()
        });
        let notifications = ContributionDescriptor::builtin(
            NOTIFICATION_ID,
            ContributionKind::Notification,
            "RSS 更新提醒",
            "rss",
            true,
            8,
        )
        .with_capabilities(vec![
            crate::extension::Capability::Event,
            crate::extension::Capability::Action,
        ]);
        let mut descriptor = ExtensionDescriptor::builtin(
            ID,
            "RSS",
            "订阅与阅读 RSS / Atom",
            vec![widget, notifications],
        );
        descriptor.settings = vec![SettingSchema::new(
            "refresh_minutes",
            SettingValue::Integer(10),
            "刷新间隔（分钟）",
        )
        .range(1.0, 180.0)];
        descriptor
    }

    fn on_start(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        let storage = ctx.sdk().storage().data().clone();
        self.feeds = storage
            .read_json("subscriptions.json")
            .map_err(|error| error.to_string())?
            .unwrap_or_default();
        self.selected = self.feeds.first().map(|feed| feed.url.clone());
        self.storage = Some(storage);
        self.fetcher = Some(Fetcher::start(ctx.sdk().tasks())?);
        self.next_refresh = Instant::now();
        Ok(())
    }

    fn on_stop(&mut self, _: &ExtensionContext) -> Result<(), String> {
        self.fetcher = None;
        self.pending.clear();
        self.reader = None;
        Ok(())
    }

    fn widget_view(&self, id: &ExtensionId) -> Option<WidgetView> {
        (id.as_str() == WIDGET_ID).then(|| view::build(self))
    }

    fn update(&mut self, now: Instant) -> ExtensionOutput {
        let mut changed = false;
        let mut notifications = Vec::new();
        if now >= self.next_refresh {
            for url in self
                .feeds
                .iter()
                .map(|feed| feed.url.clone())
                .collect::<Vec<_>>()
            {
                self.request(url);
            }
            self.next_refresh = now + self.refresh_interval;
            changed = !self.feeds.is_empty();
        }
        let results: Vec<_> = self
            .fetcher
            .as_ref()
            .map(|fetcher| fetcher.results.try_iter().collect())
            .unwrap_or_default();
        for (url, result) in results {
            self.pending.remove(&url);
            let Some(index) = self.feeds.iter().position(|feed| feed.url == url) else {
                continue;
            };
            match result {
                Ok(feed) => match self.receive_feed(index, feed) {
                    Ok(notification) => {
                        self.errors.remove(&url);
                        notifications.extend(notification);
                    }
                    Err(error) => {
                        self.errors.insert(url, error);
                    }
                },
                Err(error) => {
                    self.errors.insert(url, error);
                }
            }
            changed = true;
        }
        if changed {
            let mut output = self.changed();
            output.notifications = notifications;
            output
        } else {
            ExtensionOutput::default()
        }
    }

    fn on_action(&mut self, action: Action, _: Instant) -> ExtensionOutput {
        self.error = None;
        let mut resolved_notifications = Vec::new();
        match action {
            Action::SetText { key, value } if key == "rss-url" => self.draft = value,
            Action::Select(url) if self.feeds.iter().any(|feed| feed.url == url) => {
                self.selected = Some(url)
            }
            Action::Advance => {
                if let Some(url) = &self.selected {
                    self.request(url.clone());
                }
            }
            Action::Custom(key) if key == "add" => self.adding = true,
            Action::Custom(key) if key == "cancel" => self.adding = false,
            Action::Custom(key) if key == "subscribe" => self.error = self.subscribe().err(),
            Action::Custom(key) if key.starts_with("remove-feed:") => {
                let url = &key["remove-feed:".len()..];
                let old = self.feeds.clone();
                self.feeds.retain(|feed| feed.url != url);
                if let Err(error) = self.save() {
                    self.feeds = old;
                    self.error = Some(error);
                } else {
                    self.errors.remove(url);
                    self.pending.remove(url);
                    if self.selected.as_deref() == Some(url) {
                        self.selected = self.feeds.first().map(|feed| feed.url.clone());
                    }
                }
            }
            Action::Custom(key) if key == "unsubscribe" => {
                if let Some(url) = self.selected.clone() {
                    let old = self.feeds.clone();
                    self.feeds.retain(|feed| feed.url != url);
                    if let Err(error) = self.save() {
                        self.feeds = old;
                        self.error = Some(error);
                    } else {
                        self.errors.remove(&url);
                        self.selected = self.feeds.first().map(|feed| feed.url.clone());
                    }
                }
            }
            Action::Custom(key) if key.starts_with("open:") => {
                let target = self
                    .selected
                    .as_ref()
                    .and_then(|url| self.target(url, &key[5..]));
                if let Some(target) = target {
                    self.error = self.open_article(&target).err();
                    if self.error.is_none() {
                        resolved_notifications.push(NotificationKey {
                            source_extension_id: ExtensionId::builtin(NOTIFICATION_ID),
                            notification_id: notification_id(&target.url, &target.article.id),
                        });
                    }
                }
            }
            Action::Custom(key) if key.starts_with("read:") => {
                if let Some(url) = self.selected.clone() {
                    let id = &key[5..];
                    let was_read = self
                        .feeds
                        .iter()
                        .find(|feed| feed.url == url)
                        .is_some_and(|feed| feed.read_ids.contains(id));
                    self.error = self.mark_read(&url, id, !was_read).err();
                    if !was_read && self.error.is_none() {
                        resolved_notifications.push(NotificationKey {
                            source_extension_id: ExtensionId::builtin(NOTIFICATION_ID),
                            notification_id: notification_id(&url, id),
                        });
                    }
                }
            }
            _ => return ExtensionOutput::default(),
        }
        let mut output = self.changed();
        output.resolved_notifications = resolved_notifications;
        output
    }

    fn on_settings_changed(
        &mut self,
        _: &ExtensionContext,
        _: &ExtensionId,
        settings: &Settings,
        _: ReloadPolicy,
    ) -> ExtensionOutput {
        if let Some(SettingValue::Integer(minutes)) = settings.get("refresh_minutes") {
            self.refresh_interval = Duration::from_secs(*minutes as u64 * 60);
            self.next_refresh = Instant::now() + self.refresh_interval;
        }
        self.changed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, path::PathBuf, thread};

    fn incoming(count: usize) -> Feed {
        let items = (0..count).rev().map(|index| format!(
            "<item><guid>{index}</guid><title>Article {index}</title><link>https://example.com/{index}</link><description>Content</description></item>"
        )).collect::<String>();
        let xml = format!("<rss version=\"2.0\"><channel><title>Example</title><link>https://example.com</link><description>Test</description>{items}</channel></rss>");
        feed::parse(xml.as_bytes(), "https://example.com/rss").unwrap()
    }

    fn with_empty_feed(directory: PathBuf) -> RssExtension {
        let mut feed = incoming(0);
        feed.initialized = false;
        RssExtension {
            selected: Some(feed.url.clone()),
            feeds: vec![feed],
            storage: Some(StorageArea::from_root(
                crate::sdk::storage::AreaKind::Data,
                directory,
            )),
            ..Default::default()
        }
    }

    #[test]
    fn new_articles_publish_one_notification_with_a_view_action_and_do_not_repeat() {
        let root = tempfile::tempdir().unwrap();
        let mut extension = with_empty_feed(root.path().to_owned());
        assert!(extension.receive_feed(0, incoming(15)).unwrap().is_none());
        assert_eq!(extension.feeds[0].unread_count(), 10);
        let notification = extension.receive_feed(0, incoming(17)).unwrap().unwrap();
        assert!(extension.handles_notification(&notification.source_id));
        assert!(notification.title.contains("2 篇新文章"));
        assert_eq!(notification.actions[0].label, "查看");
        let target: ReaderTarget =
            serde_json::from_str(notification.actions[0].data.as_deref().unwrap()).unwrap();
        assert_eq!(target.article.id, "16");
        assert_eq!(target.url, extension.feeds[0].url);
        assert!(extension.receive_feed(0, incoming(17)).unwrap().is_none());
        extension.feeds = extension
            .storage
            .as_ref()
            .unwrap()
            .read_json("subscriptions.json")
            .unwrap()
            .unwrap();
        assert!(extension.receive_feed(0, incoming(17)).unwrap().is_none());
    }

    #[test]
    fn read_and_unread_buttons_persist_state_and_resolve_the_matching_update() {
        let root = tempfile::tempdir().unwrap();
        let mut extension = with_empty_feed(root.path().to_owned());
        extension.receive_feed(0, incoming(1)).unwrap();
        let notification = extension.receive_feed(0, incoming(2)).unwrap().unwrap();
        let output = extension.on_action(Action::Custom("read:1".into()), Instant::now());
        assert_eq!(output.resolved_notifications, [notification.key()]);
        assert_eq!(extension.feeds[0].unread_count(), 1);
        let stored: Vec<Feed> = extension
            .storage
            .as_ref()
            .unwrap()
            .read_json("subscriptions.json")
            .unwrap()
            .unwrap();
        assert!(stored[0].read_ids.contains("1"));
        extension.on_action(Action::Custom("read:1".into()), Instant::now());
        assert_eq!(extension.feeds[0].unread_count(), 2);
        assert!(!extension.feeds[0].read_ids.contains("1"));
    }

    #[test]
    fn failed_cache_writes_keep_the_previous_baseline_so_the_update_can_retry() {
        let directory = tempfile::tempdir().unwrap();
        let mut extension = with_empty_feed(directory.path().to_owned());
        extension.receive_feed(0, incoming(2)).unwrap();
        let invalid_directory = tempfile::NamedTempFile::new().unwrap();
        extension.storage = Some(StorageArea::from_root(
            crate::sdk::storage::AreaKind::Data,
            invalid_directory.path(),
        ));
        assert!(extension.receive_feed(0, incoming(3)).is_err());
        assert_eq!(extension.feeds[0].articles.len(), 2);
        assert!(!extension.feeds[0].seen_ids.contains("2"));
        extension.storage = Some(StorageArea::from_root(
            crate::sdk::storage::AreaKind::Data,
            directory.path(),
        ));
        assert!(extension.receive_feed(0, incoming(3)).unwrap().is_some());
    }

    #[test]
    fn adding_refreshing_restoring_and_unsubscribing_work_through_extension_actions() {
        use std::{
            io::{BufRead, BufReader},
            net::TcpListener,
        };

        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}/feed.xml", server.local_addr().unwrap());
        let serving = thread::spawn(move || {
            let (mut socket, _) = server.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(&socket);
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                    break;
                }
            }
            let body = r#"<rss version="2.0"><channel><title>Local feed</title><link>https://example.com</link><description>Test</description><item><guid>one</guid><title>Saved article</title><description><![CDATA[<p>Rendered <b>HTML</b></p>]]></description></item></channel></rss>"#;
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        });
        let root = tempfile::tempdir().unwrap();
        let ctx = ExtensionContext::new(root.path().join("data"), root.path().join("config"));
        let mut extension = RssExtension::default();
        extension.on_start(&ctx).unwrap();
        extension.on_action(
            Action::SetText {
                key: "rss-url".into(),
                value: address.clone(),
            },
            Instant::now(),
        );
        extension.on_action(Action::Custom("subscribe".into()), Instant::now());
        assert!(extension.error.is_none());
        assert_eq!(extension.feeds.len(), 1);
        assert!(extension.pending.contains(&address));
        // A duplicate submission selects the existing subscription.
        extension.draft = format!("{address}#duplicate");
        extension.on_action(Action::Custom("subscribe".into()), Instant::now());
        assert_eq!(extension.feeds.len(), 1);
        let deadline = Instant::now() + Duration::from_secs(8);
        while extension.feeds[0].articles.is_empty() {
            extension.update(Instant::now());
            assert!(
                Instant::now() < deadline,
                "feed did not load: {:?}",
                extension.errors
            );
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(extension.feeds[0].title, "Local feed");
        assert!(extension.feeds[0].articles[0].html);
        assert!(extension.pending.is_empty());
        extension.on_stop(&ctx).unwrap();
        serving.join().unwrap();

        let mut restored = RssExtension::default();
        restored.on_start(&ctx).unwrap();
        assert_eq!(restored.feeds[0].url, address);
        assert_eq!(restored.feeds[0].articles[0].title, "Saved article");
        restored.on_action(Action::Custom("unsubscribe".into()), Instant::now());
        assert!(restored.feeds.is_empty());
        assert!(restored.selected.is_none());
        restored.on_stop(&ctx).unwrap();
        restored.on_start(&ctx).unwrap();
        assert!(restored.feeds.is_empty());
    }
}
