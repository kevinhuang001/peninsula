use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashSet},
    time::Duration,
};
use url::Url;

const MAX_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Article {
    pub id: String,
    pub title: String,
    pub link: Option<String>,
    pub body: String,
    pub html: bool,
    pub date: String,
    #[serde(default)]
    pub timestamp: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Feed {
    pub url: String,
    pub title: String,
    pub articles: Vec<Article>,
    #[serde(default)]
    pub initialized: bool,
    #[serde(default)]
    pub seen_ids: BTreeSet<String>,
    #[serde(default)]
    pub read_ids: BTreeSet<String>,
}

impl Feed {
    /// Keep the initial history as a baseline, then append only genuinely new items.
    pub fn merge(&self, mut incoming: Self) -> (Self, Vec<Article>) {
        incoming.read_ids = self.read_ids.clone();
        let fresh = if self.initialized {
            incoming
                .articles
                .iter()
                .filter(|article| {
                    !self.seen_ids.contains(&article.id)
                        && !self.articles.iter().any(|old| old.id == article.id)
                })
                .cloned()
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        incoming.seen_ids.extend(self.seen_ids.iter().cloned());
        incoming
            .seen_ids
            .extend(self.articles.iter().map(|article| article.id.clone()));
        if self.initialized {
            incoming.articles.retain(|article| {
                fresh.iter().any(|new| new.id == article.id)
                    || self.articles.iter().any(|old| old.id == article.id)
            });
            let present: HashSet<_> = incoming
                .articles
                .iter()
                .map(|article| article.id.clone())
                .collect();
            incoming.articles.extend(
                self.articles
                    .iter()
                    .filter(|article| !present.contains(&article.id))
                    .cloned(),
            );
            incoming
                .articles
                .sort_by_key(|article| std::cmp::Reverse(article.timestamp));
            incoming.articles.truncate(150);
        } else {
            let count = if self.articles.is_empty() {
                10
            } else {
                self.articles.len()
            };
            incoming.articles.truncate(count);
        }
        incoming.initialized = true;
        (incoming, fresh)
    }

    pub fn unread_count(&self) -> usize {
        self.articles
            .iter()
            .filter(|article| !self.read_ids.contains(&article.id))
            .count()
    }
}

pub(super) fn normalize_url(value: &str) -> Result<String, String> {
    let mut url =
        Url::parse(value.trim()).map_err(|_| "请输入完整的 RSS 或 Atom 链接".to_owned())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("订阅链接需要以 http:// 或 https:// 开头".into());
    }
    url.set_fragment(None);
    Ok(url.into())
}

pub(super) async fn fetch(url: &str) -> Result<Feed, String> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .user_agent("Peninsula/0.1 RSS")
        .build()
        .map_err(|error| error.to_string())?;
    let mut response = client
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| format!("读取订阅失败：{error}"))?;
    let base = normalize_url(response.url().as_str())?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_BYTES)
    {
        return Err("订阅内容超过 8 MB".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
        if bytes.len().saturating_add(chunk.len()) as u64 > MAX_BYTES {
            return Err("订阅内容超过 8 MB".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let mut feed = parse(&bytes, &base)?;
    feed.url = url.into();
    Ok(feed)
}

pub(super) fn parse(bytes: &[u8], base: &str) -> Result<Feed, String> {
    let feed = feed_rs::parser::Builder::new()
        .base_uri(Some(base))
        .build()
        .parse(bytes)
        .map_err(|_| "这个链接没有返回有效的 RSS 或 Atom 内容".to_owned())?;
    let title = feed
        .title
        .map(|title| title.content)
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| base.into());
    let mut seen = HashSet::new();
    let mut articles: Vec<_> = feed
        .entries
        .into_iter()
        .filter(|entry| seen.insert(entry.id.clone()))
        .map(|entry| {
            let published = entry.published.or(entry.updated);
            let link = entry
                .links
                .iter()
                .find(|link| {
                    link.rel.as_deref().is_none_or(|rel| rel == "alternate")
                        && normalize_url(&link.href).is_ok()
                })
                .map(|link| link.href.clone());
            let (body, content_type) = match entry.content.and_then(|content| {
                content
                    .body
                    .map(|body| (body, content.content_type.to_string()))
            }) {
                Some(body) => body,
                None => entry
                    .summary
                    .map(|summary| (summary.content, summary.content_type.to_string()))
                    .unwrap_or_default(),
            };
            Article {
                id: entry.id,
                title: entry
                    .title
                    .map(|title| title.content)
                    .filter(|title| !title.trim().is_empty())
                    .unwrap_or_else(|| "无标题文章".into()),
                link,
                body,
                html: content_type.contains("html"),
                date: published
                    .map(|date| date.format("%Y-%m-%d").to_string())
                    .unwrap_or_default(),
                timestamp: published.map(|date| date.timestamp()),
            }
        })
        .collect();
    articles.sort_by_key(|article| std::cmp::Reverse(article.timestamp));
    let seen_ids = articles.iter().map(|article| article.id.clone()).collect();
    articles.truncate(150);
    Ok(Feed {
        url: base.into(),
        title,
        articles,
        initialized: true,
        seen_ids,
        read_ids: BTreeSet::new(),
    })
}

pub(super) fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub(super) fn article_html(article: &Article, source: &str) -> String {
    let content = if article.html {
        article.body.clone()
    } else {
        format!("<pre>{}</pre>", escape(&article.body))
    };
    let link = article
        .link
        .as_ref()
        .map(|url| format!("<a href=\"{}\">阅读原文 ↗</a>", escape(url)))
        .unwrap_or_default();
    format!(
        r##"<!doctype html><html lang="zh-CN"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src https: http: data:; media-src https: http:; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'">
<title>{title}</title><style>
:root{{color-scheme:light dark}}*{{box-sizing:border-box}}body{{margin:0;background:#faf9f6;color:#24272c;font:17px/1.85 -apple-system,BlinkMacSystemFont,sans-serif}}
main{{max-width:760px;margin:auto;padding:48px 36px 80px}}header{{margin-bottom:32px;border-bottom:1px solid #deded8;padding-bottom:24px}}
h1{{font-size:32px;line-height:1.35;letter-spacing:-.6px;margin:12px 0 20px}}.meta{{color:#69727e;font-size:13px}}a{{color:#256caa;text-decoration:none}}a:hover{{text-decoration:underline}}
img,video{{max-width:100%;height:auto;border-radius:8px}}pre{{white-space:pre-wrap;overflow-wrap:anywhere;font:inherit}}code{{font:14px/1.6 ui-monospace,monospace}}blockquote{{margin-left:0;padding-left:20px;border-left:3px solid #a1b1c0;color:#637081}}
table{{border-collapse:collapse;max-width:100%;display:block;overflow-x:auto}}td,th{{padding:8px;border:1px solid #89939b}}article{{overflow-wrap:anywhere}}article pre:has(code){{padding:18px;background:#e9edf0;border-radius:8px}}
@media(prefers-color-scheme:dark){{body{{background:#141a23;color:#e4e9ef}}header{{border-color:#303b49}}.meta,blockquote{{color:#a3b1c1}}a{{color:#8ec7ff}}article pre:has(code){{background:#202b38}}}}
</style></head><body><main><header><div class="meta">{source} · {date}</div><h1>{title}</h1>{link}</header><article>{content}</article></main></body></html>"##,
        title = escape(&article.title),
        source = escape(source),
        date = escape(&article.date)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn sample(count: usize) -> Feed {
        let articles: Vec<_> = (0..count)
            .rev()
            .map(|index| Article {
                id: index.to_string(),
                title: format!("Article {index}"),
                link: Some(format!("https://example.com/{index}")),
                body: "<p>Body</p>".into(),
                html: true,
                date: String::new(),
                timestamp: Some(index as i64),
            })
            .collect();
        Feed {
            url: "https://example.com/rss".into(),
            title: "Example".into(),
            seen_ids: articles.iter().map(|article| article.id.clone()).collect(),
            articles,
            initialized: true,
            read_ids: BTreeSet::new(),
        }
    }

    #[test]
    fn initial_ten_are_unread_and_refresh_adds_only_new_items_without_losing_read_state() {
        let mut initial = sample(0);
        initial.initialized = false;
        let (mut baseline, notifications) = initial.merge(sample(15));
        assert!(notifications.is_empty());
        assert_eq!(baseline.articles.len(), 10);
        assert_eq!(baseline.articles[0].id, "14");
        assert_eq!(baseline.articles[9].id, "5");
        assert_eq!(baseline.unread_count(), 10);
        baseline.read_ids.insert("14".into());
        let (unchanged, notifications) = baseline.merge(sample(15));
        assert!(notifications.is_empty());
        assert_eq!(unchanged.articles.len(), 10);
        let (updated, notifications) = unchanged.merge(sample(16));
        assert_eq!(
            notifications
                .iter()
                .map(|article| article.id.as_str())
                .collect::<Vec<_>>(),
            ["15"]
        );
        assert_eq!(updated.articles.len(), 11);
        assert!(updated.read_ids.contains("14"));
        assert_eq!(updated.unread_count(), 10);
        let restored: Feed =
            serde_json::from_str(&serde_json::to_string(&updated).unwrap()).unwrap();
        let (restored, notifications) = restored.merge(sample(16));
        assert!(notifications.is_empty());
        assert!(restored.read_ids.contains("14"));
    }

    #[test]
    fn timestamps_determine_the_latest_ten_even_when_the_feed_lists_oldest_first() {
        let items = (1..=15).map(|day| format!(
            "<item><guid>{day}</guid><title>Article {day}</title><pubDate>{day:02} Nov 2026 00:00:00 +0000</pubDate></item>"
        )).collect::<String>();
        let xml = format!("<rss version=\"2.0\"><channel><title>Example</title><link>https://example.com</link><description>Test</description>{items}</channel></rss>");
        let parsed = parse(xml.as_bytes(), "https://example.com/rss").unwrap();
        let mut initial = sample(0);
        initial.initialized = false;
        let (baseline, notifications) = initial.merge(parsed);
        assert!(notifications.is_empty());
        assert_eq!(baseline.articles[0].id, "15");
        assert_eq!(baseline.articles[9].id, "6");
    }

    #[test]
    fn rss_html_and_atom_plain_text_are_distinguished_and_scripts_are_removed() {
        let rss = br#"<rss version="2.0" xmlns:content="http://purl.org/rss/1.0/modules/content/"><channel><title>Example</title><link>https://example.com/</link><description>RSS</description><item><guid>first</guid><title>HTML article</title><link>https://example.com/read</link><content:encoded><![CDATA[<h2>Heading</h2><p>Hello <strong>world</strong></p><script>alert(1)</script>]]></content:encoded></item></channel></rss>"#;
        let parsed = parse(rss, "https://example.com/rss").unwrap();
        let article = &parsed.articles[0];
        assert!(article.html);
        assert!(article_html(article, "Example").contains("<strong>world</strong>"));
        assert!(!article.body.contains("script"));
        let atom = br#"<feed xmlns="http://www.w3.org/2005/Atom"><title>Atom</title><id>https://example.com/</id><updated>2026-11-19T00:00:00Z</updated><entry><id>text</id><title>Plain</title><content type="text">2 &lt; 3 &amp; 4</content><link href="/post"/></entry></feed>"#;
        let parsed = parse(atom, "https://example.com/atom").unwrap();
        let article = &parsed.articles[0];
        assert!(!article.html);
        assert!(article_html(article, "Atom").contains("2 &lt; 3 &amp; 4"));
        assert_eq!(article.link.as_deref(), Some("https://example.com/post"));
    }

    #[test]
    fn subscription_urls_require_http_and_normalize_duplicate_fragments() {
        assert_eq!(
            normalize_url(" https://example.com/rss#section ").unwrap(),
            "https://example.com/rss"
        );
        assert!(normalize_url("file:///etc/passwd").is_err());
        assert!(normalize_url("javascript:alert(1)").is_err());
        assert!(parse(b"<html>Not a feed</html>", "https://example.com").is_err());
    }
}
