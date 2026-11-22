mod client;
mod model;
mod view;

use crate::{
    extension::{
        Action, CapsuleView, ContributionDescriptor, ContributionKind, ExecutionPolicy,
        ExtensionContext, ExtensionDescriptor, ExtensionId, ExtensionOutput, Notification,
        NotificationAlertView, NotificationCenterItemView, NotificationPresentationProvider,
        NotificationPriority, ReloadPolicy, RuntimeExtension, SettingSchema, SettingValue, Settings,
    },
    ui::declarative::{Overflow, ScrollAxes, WidgetBounds},
};
use model::{Asset, MarketKind, MarketQuote};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const ID: &str = "builtin.market";
const WIDGET_ID: &str = "builtin.market/overview";
const LEGACY_ALERT_ID: &str = "builtin.market/price-alerts";
const SERVICE_ID: &str = "builtin.market/service.quotes";
const CONDITION: &str = "condition";
const THRESHOLD: &str = "threshold";
const ADD_KEY: &str = "market-symbol";
const ADD_ACTION: &str = "market:add";
const ADD_OPEN_ACTION: &str = "market:add-open";
const ADD_CANCEL_ACTION: &str = "market:add-cancel";
const REMOVE_PREFIX: &str = "market:remove:";
const REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const MAX_ASSETS: usize = 24;
const DEFAULT_ALERT_INTERVAL_MINUTES: u32 = 30;
const SETTINGS_ALERT_INTERVAL: &str = "alert_interval_minutes";
const SETTINGS_ALERT_RULES: &str = "alert_rules";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Watchlist {
    #[serde(default = "default_assets")]
    assets: Vec<Asset>,
    #[serde(default)]
    alert_rules: HashMap<String, PriceAlertRule>,
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

#[derive(Deserialize)]
struct AlertSettingsRule {
    condition: AlertCondition,
    price: Option<String>,
}

impl Default for Watchlist {
    fn default() -> Self {
        Self {
            assets: default_assets(),
            alert_rules: HashMap::new(),
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
    error: Option<String>,
    pending: Option<Receiver<Vec<MarketQuote>>>,
    watchlist_path: Option<PathBuf>,
    legacy_settings: Settings,
    alert_interval_minutes: u32,
    active: BTreeMap<ExtensionId, Settings>,
    next_refresh: Instant,
    refresh_requested: bool,
    refreshing: bool,
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
            error: None,
            pending: None,
            watchlist_path: None,
            legacy_settings: Settings::new(),
            alert_interval_minutes: DEFAULT_ALERT_INTERVAL_MINUTES,
            active: BTreeMap::new(),
            next_refresh: Instant::now(),
            refresh_requested: false,
            refreshing: false,
            add_open: false,
            last_updated: None,
            last_alerts: HashMap::new(),
            pending_notifications: Vec::new(),
        }
    }

    fn start_fetch(&mut self, context: &ExtensionContext) {
        if self.pending.is_some() {
            return;
        }
        let full_watchlist = self.active.contains_key(&ExtensionId::builtin(WIDGET_ID));
        let assets: Vec<_> = self
            .watchlist
            .assets
            .iter()
            .filter(|asset| {
                full_watchlist
                    || self.active.contains_key(&capsule_item_id(asset))
                    || self.active.contains_key(&alert_id(asset))
            })
            .cloned()
            .collect();
        if assets.is_empty() {
            self.refresh_requested = false;
            self.next_refresh = Instant::now() + REFRESH_INTERVAL;
            return;
        }
        let cached_logos = self
            .quotes
            .iter()
            .filter_map(|(key, quote)| quote.logo.clone().map(|logo| (key.clone(), logo)))
            .collect();
        let (sender, receiver) = mpsc::channel();
        match context.tasks().spawn("market-quotes", move |cancel| {
            if let Ok(Some(quotes)) = crate::sdk::http::run_cancellable(
                &cancel,
                client::fetch_quotes(&assets, &cached_logos),
            ) {
                let _ = sender.send(quotes);
            }
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
                    if let (Some(current), Some(settings)) =
                        (quote.price, self.active.get(&alert_id(&quote.asset)))
                    {
                        let threshold = threshold(settings);
                        let condition = if matches!(settings.get(CONDITION), Some(SettingValue::Enum(value)) if value == "低于")
                        {
                            AlertCondition::Less
                        } else {
                            AlertCondition::Greater
                        };
                        let cooldown = Duration::from_secs(
                            u64::from(self.alert_interval_minutes) * 60,
                        );
                        let triggered = match condition {
                            AlertCondition::Greater => current > threshold,
                            AlertCondition::Less => current < threshold,
                        };
                        let cooling_down = self
                            .last_alerts
                            .get(&key)
                            .is_some_and(|last| now.saturating_duration_since(*last) < cooldown);
                        if threshold > 0.0 && triggered && !cooling_down {
                            self.pending_notifications
                                .push(price_alert(&quote, threshold, condition));
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
        let previous = self.watchlist.clone();
        self.watchlist
            .assets
            .retain(|asset| asset.kind != kind || asset.symbol != symbol);
        self.watchlist.alert_rules.remove(&key);
        if let Err(error) = self.save_watchlist() {
            self.watchlist = previous;
            self.error = Some(format!("自选列表保存失败：{error}"));
            return;
        }
        self.quotes.remove(&key);
        self.last_alerts.remove(&key);
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

fn asset_id(asset: &Asset) -> String {
    // Underscores are not valid market symbols. This mapping is injective for
    // normalized symbols, including index symbols that begin with a caret.
    let symbol = asset.symbol.replace('^', "_");
    format!("{ID}/assets/{}/{symbol}", asset.kind.key())
}

pub(in crate::builtins::market) fn capsule_item_id(asset: &Asset) -> ExtensionId {
    ExtensionId::new(asset_id(asset)).expect("normalized market asset IDs are valid")
}

fn alert_id(asset: &Asset) -> ExtensionId {
    ExtensionId::new(format!("{}/price-alert", asset_id(asset))).expect("normalized alert ID")
}

fn threshold(settings: &Settings) -> f64 {
    match settings.get(THRESHOLD) {
        Some(SettingValue::Float(value)) => *value,
        _ => 0.0,
    }
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
        let service_id = ExtensionId::builtin(SERVICE_ID);
        let service = ContributionDescriptor::builtin(
            SERVICE_ID,
            ContributionKind::BackgroundService,
            "行情服务",
            "chart",
            true,
            0,
        )
        .with_execution_policy(ExecutionPolicy::Interval(Duration::from_millis(250)));
        let mut descriptor = ExtensionDescriptor::builtin(
            ID,
            "市场",
            "股票与加密资产行情自选表格",
            vec![widget.with_dependency(service_id), service],
        );
        descriptor.execution_policy = ExecutionPolicy::EventDriven;
        descriptor.settings = vec![
            SettingSchema::new(
                SETTINGS_ALERT_INTERVAL,
                SettingValue::Integer(i64::from(DEFAULT_ALERT_INTERVAL_MINUTES)),
                "统一最短提醒间隔",
            )
            .range(1.0, 10_080.0)
            .with_unit("分钟")
            .in_notifications(),
        ];
        descriptor
    }

    fn on_settings_changed(
        &mut self,
        _: &ExtensionContext,
        _: &ExtensionId,
        values: &Settings,
        _: ReloadPolicy,
    ) -> ExtensionOutput {
        if let Some(SettingValue::Integer(minutes)) = values.get(SETTINGS_ALERT_INTERVAL) {
            self.alert_interval_minutes = (*minutes).clamp(1, 10_080) as u32;
        }
        ExtensionOutput::default()
    }

    fn import_legacy_settings(&mut self, settings: &Settings) {
        self.legacy_settings.clone_from(settings);
    }

    fn on_contribution_state(
        &mut self,
        _: &ExtensionContext,
        active: &BTreeMap<ExtensionId, Settings>,
    ) {
        if self.active.keys().ne(active.keys()) {
            self.refresh_requested = true;
        }
        self.active.clone_from(active);
    }

    fn on_contribution_stop(
        &mut self,
        _: &ExtensionContext,
        id: &ExtensionId,
    ) -> Result<(), String> {
        if id.as_str() == SERVICE_ID {
            self.pending = None;
            self.pending_notifications.clear();
            self.refreshing = false;
        }
        Ok(())
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
        if let Some(SettingValue::Integer(minutes)) =
            self.legacy_settings.get(SETTINGS_ALERT_INTERVAL)
        {
            self.alert_interval_minutes = (*minutes).clamp(1, 10_080) as u32;
        }
        if let Some(SettingValue::String(source)) = self.legacy_settings.get(SETTINGS_ALERT_RULES) {
            if let Ok(rules) = serde_json::from_str::<HashMap<String, AlertSettingsRule>>(source) {
                self.watchlist.alert_rules = self
                    .watchlist
                    .assets
                    .iter()
                    .filter_map(|asset| {
                        let key = asset.cache_key();
                        let rule = rules.get(&key)?;
                        let price = rule
                            .price
                            .as_deref()?
                            .trim()
                            .parse::<f64>()
                            .ok()
                            .filter(|price| price.is_finite() && *price > 0.0)?;
                        Some((
                            key,
                            PriceAlertRule {
                                condition: rule.condition,
                                price: Some(price),
                            },
                        ))
                    })
                    .collect();
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

    fn on_contribution_update(
        &mut self,
        context: &ExtensionContext,
        id: &ExtensionId,
        now: Instant,
    ) -> ExtensionOutput {
        if id.as_str() != SERVICE_ID {
            return ExtensionOutput::default();
        }
        let mut changed = self.poll_fetch(now);
        if self.pending.is_none() && (self.refresh_requested || now >= self.next_refresh) {
            self.start_fetch(context);
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

    fn capsule_refresh_interval(&self) -> Option<Duration> {
        Some(REFRESH_INTERVAL)
    }

    fn capsule_views(&self) -> Vec<CapsuleView> {
        view::capsule(self)
    }

    fn dynamic_contributions(&self) -> Option<Vec<ContributionDescriptor>> {
        let service = ExtensionId::builtin(SERVICE_ID);
        Some(
            self.watchlist
                .assets
                .iter()
                .flat_map(|asset| {
                    let legacy = self.watchlist.alert_rules.get(&asset.cache_key());
                    let price = legacy.and_then(|rule| rule.price).unwrap_or(0.0);
                    let condition = if legacy
                        .is_some_and(|rule| matches!(rule.condition, AlertCondition::Less))
                    {
                        "低于"
                    } else {
                        "高于"
                    };
                    let capsule = ContributionDescriptor::new(
                        capsule_item_id(asset),
                        ContributionKind::Capsule,
                        asset.symbol.clone(),
                        "chart",
                        false,
                        100,
                    )
                    .with_dependency(service.clone());
                    let notification = ContributionDescriptor::new(
                        alert_id(asset),
                        ContributionKind::Notification,
                        format!("{} 价格提醒", asset.symbol),
                        "bell",
                        price > 0.0,
                        100,
                    )
                    .with_dependency(service.clone())
                    .with_group(vec![match asset.kind {
                        MarketKind::Stock => "股票".into(),
                        MarketKind::Crypto => "加密货币".into(),
                    }])
                    .with_setup(true)
                    .with_settings(
                        1,
                        vec![
                            SettingSchema {
                                options: vec!["高于".into(), "低于".into()],
                                ..SettingSchema::new(
                                    CONDITION,
                                    SettingValue::Enum(condition.into()),
                                    "触发条件",
                                )
                            },
                            SettingSchema {
                                constraints: crate::extension::Constraints {
                                    min: Some(0.0),
                                    ..Default::default()
                                },
                                ..SettingSchema::new(
                                    THRESHOLD,
                                    SettingValue::Float(price),
                                    "目标价格",
                                )
                                .number_text("例如 150.50")
                                .activation_positive()
                            },
                        ],
                    );
                    [capsule, notification]
                })
                .collect(),
        )
    }

    fn on_action(&mut self, action: Action, _now: Instant) -> ExtensionOutput {
        match action {
            Action::SetText { key, value } if key == ADD_KEY => {
                self.draft = value;
                self.error = None;
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
        source_id.as_str() == LEGACY_ALERT_ID
            || self
                .watchlist
                .assets
                .iter()
                .any(|asset| alert_id(asset) == *source_id)
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
        source_id: alert_id(&quote.asset),
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

#[cfg(test)]
mod notification_contract_tests {
    use super::*;

    #[test]
    fn market_notifications_are_per_asset_with_a_single_extension_cooldown() {
        let market = MarketExtension::new();
        let descriptor = market.descriptor();
        assert!(descriptor.settings.iter().any(|field| {
            field.key == SETTINGS_ALERT_INTERVAL
                && matches!(field.default, Some(SettingValue::Integer(30)))
        }));
        let entries = market.dynamic_contributions().expect("market asset contributions");
        let notifications: Vec<_> = entries.iter()
            .filter(|entry| entry.kind == ContributionKind::Notification).collect();
        assert_eq!(notifications.len(), market.watchlist.assets.len());
        for entry in notifications {
            assert!(entry.id.as_str().ends_with("/price-alert"));
            assert!(entry.settings.iter().any(|field| field.key == CONDITION));
            assert!(entry.settings.iter().any(|field| field.key == THRESHOLD));
            assert!(entry.settings.iter().all(|field| field.key != "cooldown"));
            assert!(entry.dependencies.contains(&ExtensionId::builtin(SERVICE_ID)));
        }
    }
}
