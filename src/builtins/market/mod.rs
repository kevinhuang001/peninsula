mod client;
mod model;
mod view;

use crate::{
    extension::{
        Action, CompactView, ContributionDescriptor, ContributionKind, ExtensionContext,
        ExtensionDescriptor, ExtensionId, ExtensionOutput, Notification, NotificationAlertView,
        NotificationCenterItemView, NotificationPresentationProvider, NotificationPriority,
        RuntimeExtension,
    },
    ui::declarative::{Overflow, ScrollAxes, WidgetBounds},
};
use model::{Asset, MarketKind, MarketQuote};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const ID: &str = "builtin.market";
const WIDGET_ID: &str = "builtin.market/overview";
const ALERT_ID: &str = "builtin.market/price-alerts";
const ADD_KEY: &str = "market-symbol";
const ALERT_INTERVAL_KEY: &str = "market-alert-interval";
const ALERT_PRICE_PREFIX: &str = "market-alert-price:";
const ALERT_CONDITION_PREFIX: &str = "market-alert-condition:";
const ADD_ACTION: &str = "market:add";
const ADD_OPEN_ACTION: &str = "market:add-open";
const ADD_CANCEL_ACTION: &str = "market:add-cancel";
const SETTINGS_ACTION: &str = "market:settings";
const REMOVE_PREFIX: &str = "market:remove:";
const REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const MAX_ASSETS: usize = 24;
const DEFAULT_ALERT_INTERVAL_MINUTES: u32 = 30;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Watchlist {
    #[serde(default = "default_assets")]
    assets: Vec<Asset>,
    #[serde(default)]
    alert_rules: HashMap<String, PriceAlertRule>,
    #[serde(default = "default_alert_interval")]
    alert_interval_minutes: u32,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
enum AlertCondition {
    #[default]
    Greater,
    Less,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PriceAlertRule {
    condition: AlertCondition,
    price: Option<f64>,
}

fn default_alert_interval() -> u32 {
    DEFAULT_ALERT_INTERVAL_MINUTES
}

impl Default for Watchlist {
    fn default() -> Self {
        Self {
            assets: default_assets(),
            alert_rules: HashMap::new(),
            alert_interval_minutes: DEFAULT_ALERT_INTERVAL_MINUTES,
        }
    }
}

fn default_assets() -> Vec<Asset> {
    [
        (MarketKind::Stock, "AAPL"),
        (MarketKind::Stock, "NVDA"),
        (MarketKind::Stock, "TSLA"),
        (MarketKind::Crypto, "BTC"),
        (MarketKind::Crypto, "ETH"),
        (MarketKind::Crypto, "SOL"),
    ]
    .into_iter()
    .map(|(kind, symbol)| Asset {
        kind,
        symbol: symbol.into(),
    })
    .collect()
}

pub struct MarketExtension {
    watchlist: Watchlist,
    quotes: HashMap<String, MarketQuote>,
    draft: String,
    alert_price_drafts: HashMap<String, String>,
    alert_interval_draft: Option<String>,
    error: Option<String>,
    pending: Option<Receiver<Vec<MarketQuote>>>,
    watchlist_path: Option<PathBuf>,
    next_refresh: Instant,
    refresh_requested: bool,
    refreshing: bool,
    settings_open: bool,
    add_open: bool,
    last_updated: Option<String>,
    last_alerts: HashMap<String, Instant>,
    pending_notifications: Vec<Notification>,
}

impl MarketExtension {
    pub fn new() -> Self {
        let watchlist = Watchlist::default();
        Self {
            quotes: loading_quotes(&watchlist.assets),
            watchlist,
            draft: String::new(),
            alert_price_drafts: HashMap::new(),
            alert_interval_draft: None,
            error: None,
            pending: None,
            watchlist_path: None,
            next_refresh: Instant::now(),
            refresh_requested: false,
            refreshing: false,
            settings_open: false,
            add_open: false,
            last_updated: None,
            last_alerts: HashMap::new(),
            pending_notifications: Vec::new(),
        }
    }

    fn start_fetch(&mut self) {
        if self.pending.is_some() {
            return;
        }
        let assets = self.watchlist.assets.clone();
        let cached_logos = self
            .quotes
            .iter()
            .filter_map(|(key, quote)| quote.logo.clone().map(|logo| (key.clone(), logo)))
            .collect();
        let (sender, receiver) = mpsc::channel();
        match thread::Builder::new()
            .name("market-quotes".into())
            .spawn(move || {
                let _ = sender.send(client::fetch_quotes(&assets, &cached_logos));
            }) {
            Ok(_) => {
                self.pending = Some(receiver);
                self.refresh_requested = false;
                self.refreshing = true;
                self.error = None;
            }
            Err(error) => {
                self.error = Some(format!("无法启动行情查询：{error}"));
                self.next_refresh = Instant::now() + REFRESH_INTERVAL;
            }
        }
    }

    fn poll_fetch(&mut self, now: Instant) -> bool {
        let Some(receiver) = &self.pending else {
            return false;
        };
        match receiver.try_recv() {
            Ok(quotes) => {
                self.pending = None;
                self.refreshing = false;
                for quote in quotes {
                    let key = quote.asset.cache_key();
                    let rule = self.watchlist.alert_rules.get(&key);
                    if let (Some(current), Some(rule), Some(threshold)) =
                        (quote.price, rule, rule.and_then(|rule| rule.price))
                    {
                        let triggered = match rule.condition {
                            AlertCondition::Greater => current > threshold,
                            AlertCondition::Less => current < threshold,
                        };
                        let cooling_down = self.last_alerts.get(&key).is_some_and(|last| {
                            now.duration_since(*last)
                                < Duration::from_secs(
                                    u64::from(self.watchlist.alert_interval_minutes) * 60,
                                )
                        });
                        if triggered && !cooling_down {
                            self.pending_notifications.push(price_alert(
                                &quote,
                                threshold,
                                rule.condition,
                            ));
                            self.last_alerts.insert(key.clone(), now);
                        }
                    }
                    self.quotes.insert(key, quote);
                }
                self.retain_current_quotes();
                self.last_updated = Some(local_time());
                self.next_refresh = now + REFRESH_INTERVAL;
                true
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                self.pending = None;
                self.refreshing = false;
                self.error = Some("行情服务暂时中断".into());
                self.next_refresh = now + REFRESH_INTERVAL;
                true
            }
        }
    }

    fn retain_current_quotes(&mut self) {
        self.quotes.retain(|key, _| {
            self.watchlist
                .assets
                .iter()
                .any(|asset| asset.cache_key() == *key)
        });
    }

    fn add_asset(&mut self) {
        let asset = match parse_asset(&self.draft) {
            Ok(asset) => asset,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        if self.watchlist.assets.contains(&asset) {
            self.error = Some(format!("{} 已在自选列表中", asset.symbol));
            return;
        }
        if self.watchlist.assets.len() >= MAX_ASSETS {
            self.error = Some(format!("自选列表最多添加 {MAX_ASSETS} 个代码"));
            return;
        }
        let key = asset.cache_key();
        self.watchlist.assets.push(asset.clone());
        self.quotes.insert(key.clone(), MarketQuote::loading(asset));
        if let Err(error) = self.save_watchlist() {
            self.watchlist.assets.pop();
            self.quotes.remove(&key);
            self.error = Some(format!("自选列表保存失败：{error}"));
            return;
        }
        self.error = None;
        self.draft.clear();
        self.refresh_requested = true;
    }

    fn remove_asset(&mut self, kind: MarketKind, symbol: &str) {
        let key = format!("{}:{symbol}", kind.key());
        self.watchlist
            .assets
            .retain(|asset| asset.kind != kind || asset.symbol != symbol);
        self.quotes.remove(&key);
        self.watchlist.alert_rules.remove(&key);
        self.last_alerts.remove(&key);
        self.alert_price_drafts.remove(&key);
        if let Err(error) = self.save_watchlist() {
            self.error = Some(format!("自选列表保存失败：{error}"));
        }
    }

    fn set_alert_price(&mut self, key: &str, value: &str) {
        self.alert_price_drafts
            .insert(key.to_owned(), value.to_owned());
        let value = value.trim();
        if value.is_empty() {
            if let Some(rule) = self.watchlist.alert_rules.get_mut(key) {
                rule.price = None;
                if let Err(error) = self.save_watchlist() {
                    self.error = Some(format!("提醒价格保存失败：{error}"));
                }
            }
            return;
        }
        let Ok(price) = value.parse::<f64>() else {
            return;
        };
        if !price.is_finite() || price <= 0.0 {
            return;
        }
        let rule = self
            .watchlist
            .alert_rules
            .entry(key.to_owned())
            .or_insert(PriceAlertRule {
                condition: AlertCondition::Greater,
                price: None,
            });
        if rule.price == Some(price) {
            return;
        }
        rule.price = Some(price);
        if let Err(error) = self.save_watchlist() {
            self.error = Some(format!("提醒价格保存失败：{error}"));
        }
    }

    fn toggle_alert_condition(&mut self, key: &str) {
        let rule = self
            .watchlist
            .alert_rules
            .entry(key.to_owned())
            .or_insert(PriceAlertRule {
                condition: AlertCondition::Greater,
                price: None,
            });
        rule.condition = match rule.condition {
            AlertCondition::Greater => AlertCondition::Less,
            AlertCondition::Less => AlertCondition::Greater,
        };
        if let Err(error) = self.save_watchlist() {
            self.error = Some(format!("提醒条件保存失败：{error}"));
        }
    }

    fn set_alert_interval(&mut self, value: &str) {
        self.alert_interval_draft = Some(value.to_owned());
        let Ok(minutes) = value.trim().parse::<u32>() else {
            return;
        };
        if !(1..=10_080).contains(&minutes) || minutes == self.watchlist.alert_interval_minutes {
            return;
        }
        self.watchlist.alert_interval_minutes = minutes;
        if let Err(error) = self.save_watchlist() {
            self.error = Some(format!("提醒间隔保存失败：{error}"));
        }
    }

    fn save_watchlist(&self) -> Result<(), String> {
        let Some(path) = &self.watchlist_path else {
            return Err("设置目录尚未就绪".into());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let bytes =
            serde_json::to_vec_pretty(&self.watchlist).map_err(|error| error.to_string())?;
        let directory = path.parent().ok_or_else(|| "设置目录无效".to_owned())?;
        let mut temporary =
            tempfile::NamedTempFile::new_in(directory).map_err(|error| error.to_string())?;
        std::io::Write::write_all(&mut temporary, &bytes).map_err(|error| error.to_string())?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|error| error.to_string())?;
        temporary
            .persist(path)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

pub(in crate::builtins::market) fn compact_item_id(asset: &Asset) -> ExtensionId {
    ExtensionId::new(format!("{ID}/assets/{}/{}", asset.kind.key(), asset.symbol))
        .expect("normalized market asset IDs are valid")
}

impl Default for MarketExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeExtension for MarketExtension {
    fn descriptor(&self) -> ExtensionDescriptor {
        let widget = ContributionDescriptor::builtin(
            WIDGET_ID,
            ContributionKind::Widget,
            "市场行情",
            "chart",
            true,
            6,
        )
        .with_size_hint(560.0, 430.0)
        .with_widget_bounds(WidgetBounds {
            min: [430.0, 290.0],
            max: [Some(780.0), Some(690.0)],
            resizable: true,
            fit_height: true,
            overflow: Overflow::Scroll(ScrollAxes::Vertical),
            ..Default::default()
        });
        let alerts = ContributionDescriptor::builtin(
            ALERT_ID,
            ContributionKind::Notification,
            "股票价格提醒",
            "chart",
            true,
            10,
        );
        ExtensionDescriptor::builtin(
            ID,
            "市场",
            "股票与加密资产行情自选表格",
            vec![widget, alerts],
        )
    }

    fn on_start(&mut self, context: &ExtensionContext) -> Result<(), String> {
        let path = context
            .extension_data_dir(&ExtensionId::builtin(ID))
            .join("watchlist.json");
        if let Ok(file) = fs::File::open(&path) {
            if let Ok(watchlist) = serde_json::from_reader::<_, Watchlist>(file) {
                self.watchlist = watchlist;
                self.watchlist.assets =
                    normalize_assets(std::mem::take(&mut self.watchlist.assets));
                self.watchlist.alert_rules.retain(|key, rule| {
                    rule.price
                        .is_none_or(|price| price.is_finite() && price > 0.0)
                        && self
                            .watchlist
                            .assets
                            .iter()
                            .any(|asset| asset.cache_key() == *key)
                });
            }
        }
        self.watchlist_path = Some(path);
        self.quotes = loading_quotes(&self.watchlist.assets);
        self.next_refresh = Instant::now();
        Ok(())
    }

    fn on_stop(&mut self, _: &ExtensionContext) -> Result<(), String> {
        self.pending = None;
        self.refreshing = false;
        Ok(())
    }

    fn update(&mut self, now: Instant) -> ExtensionOutput {
        let mut changed = self.poll_fetch(now);
        if self.pending.is_none() && (self.refresh_requested || now >= self.next_refresh) {
            self.start_fetch();
            changed = true;
        }
        ExtensionOutput {
            invalidated: changed
                .then(|| ExtensionId::builtin(WIDGET_ID))
                .into_iter()
                .collect(),
            notifications: std::mem::take(&mut self.pending_notifications),
            ..Default::default()
        }
    }

    fn widget_view(&self, widget_id: &ExtensionId) -> Option<crate::extension::WidgetView> {
        (widget_id.as_str() == WIDGET_ID).then(|| view::build(self))
    }

    fn compact_refresh_interval(&self) -> Option<Duration> {
        Some(REFRESH_INTERVAL)
    }

    fn compact_views(&self) -> Vec<CompactView> {
        view::compact(self)
    }

    fn dynamic_compact_items(&self) -> Option<Vec<(ExtensionId, String)>> {
        Some(
            self.watchlist
                .assets
                .iter()
                .map(|asset| (compact_item_id(asset), asset.symbol.clone()))
                .collect(),
        )
    }

    fn on_action(&mut self, action: Action, _now: Instant) -> ExtensionOutput {
        match action {
            Action::SetText { key, value } if key == ADD_KEY => {
                self.draft = value;
                self.error = None;
            }
            Action::SetText { key, value } if key == ALERT_INTERVAL_KEY => {
                self.set_alert_interval(&value);
            }
            Action::SetText { key, value } if key.starts_with(ALERT_PRICE_PREFIX) => {
                self.set_alert_price(&key[ALERT_PRICE_PREFIX.len()..], &value);
            }
            Action::Custom(value) if value.starts_with(ALERT_CONDITION_PREFIX) => {
                self.toggle_alert_condition(&value[ALERT_CONDITION_PREFIX.len()..]);
            }
            Action::Advance => {
                self.refresh_requested = true;
                self.error = None;
            }
            Action::Custom(value) if value == ADD_ACTION => {
                self.add_asset();
                if self.error.is_none() {
                    self.add_open = false;
                }
            }
            Action::Custom(value) if value == ADD_OPEN_ACTION => {
                self.error = None;
                self.add_open = true;
            }
            Action::Custom(value) if value == ADD_CANCEL_ACTION => {
                self.error = None;
                self.add_open = false;
            }
            Action::Custom(value) if value == SETTINGS_ACTION => {
                self.settings_open = !self.settings_open;
            }
            Action::Custom(value) if value.starts_with(REMOVE_PREFIX) => {
                let tail = &value[REMOVE_PREFIX.len()..];
                if let Some((kind, symbol)) = tail.split_once(':') {
                    let kind = match kind {
                        "stock" => Some(MarketKind::Stock),
                        "crypto" => Some(MarketKind::Crypto),
                        _ => None,
                    };
                    if let Some(kind) = kind {
                        self.remove_asset(kind, symbol);
                    }
                }
            }
            _ => return ExtensionOutput::default(),
        }
        ExtensionOutput {
            invalidated: vec![ExtensionId::builtin(WIDGET_ID)],
            ..Default::default()
        }
    }
}

impl NotificationPresentationProvider for MarketExtension {
    fn alert_view(&self, notification: &Notification) -> NotificationAlertView {
        NotificationAlertView {
            title: notification.title.clone(),
            body: notification.body.clone(),
            icon: Some("chart".into()),
            actions: notification.actions.clone(),
        }
    }

    fn center_item_view(&self, notification: &Notification) -> NotificationCenterItemView {
        NotificationCenterItemView {
            title: notification.title.clone(),
            body: notification.body.clone(),
            icon: Some("chart".into()),
        }
    }

    fn handles_notification(&self, source_id: &ExtensionId) -> bool {
        source_id.as_str() == ALERT_ID
    }
}

fn price_alert(quote: &MarketQuote, threshold: f64, condition: AlertCondition) -> Notification {
    let price = quote.price.unwrap_or(threshold);
    let operator = match condition {
        AlertCondition::Greater => ">",
        AlertCondition::Less => "<",
    };
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    Notification {
        source_id: ExtensionId::builtin(ALERT_ID),
        id: format!("{}:{nonce}", quote.asset.cache_key()),
        title: format!("{} 价格提醒", quote.asset.symbol),
        body: format!(
            "{} 当前价格 {}，已满足 {} {} 的提醒条件",
            quote.asset.symbol,
            view::format_price(price, &quote.currency),
            operator,
            view::format_price(threshold, &quote.currency),
        ),
        icon: Some("chart".into()),
        priority: NotificationPriority::Normal,
        actions: Vec::new(),
        created_at: SystemTime::now(),
    }
}

fn local_time() -> String {
    time::OffsetDateTime::now_local()
        .map(|now| format!("{:02}:{:02}:{:02}", now.hour(), now.minute(), now.second()))
        .unwrap_or_else(|_| "刚刚".into())
}

fn loading_quotes(assets: &[Asset]) -> HashMap<String, MarketQuote> {
    assets
        .iter()
        .cloned()
        .map(|asset| (asset.cache_key(), MarketQuote::loading(asset)))
        .collect()
}

fn normalize_assets(assets: Vec<Asset>) -> Vec<Asset> {
    let mut normalized = Vec::new();
    for asset in assets {
        if normalized.len() >= MAX_ASSETS {
            break;
        }
        let Ok(symbol) = normalize_symbol(asset.kind, &asset.symbol) else {
            continue;
        };
        let candidate = Asset {
            kind: asset.kind,
            symbol,
        };
        if !normalized.contains(&candidate) {
            normalized.push(candidate);
        }
    }
    normalized
}

fn parse_asset(raw: &str) -> Result<Asset, String> {
    let raw = raw.trim().to_ascii_uppercase();
    if raw.is_empty() {
        return Err("请输入股票代码或加密代码".into());
    }
    if let Some(symbol) = raw.strip_suffix("-USD") {
        return Ok(Asset {
            kind: MarketKind::Crypto,
            symbol: normalize_symbol(MarketKind::Crypto, symbol)?,
        });
    }
    let kind = if is_common_crypto(&raw) {
        MarketKind::Crypto
    } else {
        MarketKind::Stock
    };
    Ok(Asset {
        kind,
        symbol: normalize_symbol(kind, &raw)?,
    })
}

fn normalize_symbol(kind: MarketKind, raw: &str) -> Result<String, String> {
    let symbol = raw.trim().to_ascii_uppercase();
    let valid = match kind {
        MarketKind::Stock => {
            !symbol.is_empty()
                && symbol.len() <= 16
                && symbol
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'^'))
        }
        MarketKind::Crypto => {
            !symbol.is_empty()
                && symbol.len() <= 12
                && symbol.bytes().all(|byte| byte.is_ascii_alphanumeric())
        }
    };
    if valid {
        Ok(symbol)
    } else {
        Err("代码格式无效，请检查后重试".into())
    }
}

fn is_common_crypto(symbol: &str) -> bool {
    matches!(
        symbol,
        "BTC"
            | "ETH"
            | "SOL"
            | "XRP"
            | "BNB"
            | "DOGE"
            | "ADA"
            | "AVAX"
            | "LINK"
            | "DOT"
            | "LTC"
            | "BCH"
            | "TRX"
            | "SHIB"
            | "USDC"
            | "USDT"
            | "TON"
            | "SUI"
            | "UNI"
            | "XLM"
            | "ATOM"
            | "ETC"
            | "NEAR"
            | "APT"
            | "AAVE"
            | "FIL"
    )
}
