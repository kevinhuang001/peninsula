mod client;
mod model;
mod view;

use crate::{
    extension::{
        Action, ContributionDescriptor, ContributionKind, ExtensionContext, ExtensionDescriptor,
        ExtensionId, ExtensionOutput, NotificationPresentationProvider, RuntimeExtension,
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
    time::{Duration, Instant},
};

const ID: &str = "builtin.market";
const WIDGET_ID: &str = "builtin.market/overview";
const ADD_KEY: &str = "market-symbol";
const ADD_ACTION: &str = "market:add";
const REMOVE_PREFIX: &str = "market:remove:";
const REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const MAX_ASSETS: usize = 24;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Watchlist {
    #[serde(default = "default_assets")]
    assets: Vec<Asset>,
}

impl Default for Watchlist {
    fn default() -> Self {
        Self {
            assets: default_assets(),
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
    next_refresh: Instant,
    refresh_requested: bool,
    refreshing: bool,
    last_updated: Option<String>,
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
            next_refresh: Instant::now(),
            refresh_requested: false,
            refreshing: false,
            last_updated: None,
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
                    self.quotes.insert(quote.asset.cache_key(), quote);
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
        self.watchlist
            .assets
            .retain(|asset| asset.kind != kind || asset.symbol != symbol);
        self.quotes.remove(&format!("{}:{symbol}", kind.key()));
        if let Err(error) = self.save_watchlist() {
            self.error = Some(format!("自选列表保存失败：{error}"));
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

impl Default for MarketExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationPresentationProvider for MarketExtension {}

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
        ExtensionDescriptor::builtin(ID, "市场", "股票与加密资产行情自选表格", vec![widget])
    }

    fn on_start(&mut self, context: &ExtensionContext) -> Result<(), String> {
        let path = context
            .extension_data_dir(&ExtensionId::builtin(ID))
            .join("watchlist.json");
        if let Ok(file) = fs::File::open(&path) {
            if let Ok(watchlist) = serde_json::from_reader::<_, Watchlist>(file) {
                self.watchlist.assets = normalize_assets(watchlist.assets);
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
            ..Default::default()
        }
    }

    fn widget_view(&self, widget_id: &ExtensionId) -> Option<crate::extension::WidgetView> {
        (widget_id.as_str() == WIDGET_ID).then(|| view::build(self))
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
            Action::Custom(value) if value == ADD_ACTION => self.add_asset(),
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
