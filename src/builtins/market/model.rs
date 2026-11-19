use crate::ui::declarative::RasterImage;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) enum MarketKind {
    Stock,
    Crypto,
}

impl MarketKind {
    pub(super) fn key(self) -> &'static str {
        match self {
            Self::Stock => "stock",
            Self::Crypto => "crypto",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct Asset {
    pub(super) kind: MarketKind,
    pub(super) symbol: String,
}

impl Asset {
    pub(super) fn cache_key(&self) -> String {
        format!("{}:{}", self.kind.key(), self.symbol)
    }
}

#[derive(Clone, Debug)]
pub(super) struct MarketQuote {
    pub(super) asset: Asset,
    pub(super) name: String,
    pub(super) price: Option<f64>,
    pub(super) currency: String,
    pub(super) change_percent: Option<f64>,
    pub(super) logo: Option<RasterImage>,
    pub(super) error: Option<String>,
}

impl MarketQuote {
    pub(super) fn loading(asset: Asset) -> Self {
        Self {
            name: asset.symbol.clone(),
            currency: "USD".into(),
            asset,
            price: None,
            change_percent: None,
            logo: None,
            error: None,
        }
    }
}
