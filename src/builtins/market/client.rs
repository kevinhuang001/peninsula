use super::model::{Asset, MarketKind, MarketQuote};
use crate::ui::declarative::RasterImage;
use reqwest::blocking::{Client, Response};
use serde_json::Value;
use std::{collections::HashMap, thread, time::Duration};
use url::Url;

const MAX_LOGO_BYTES: u64 = 2 * 1024 * 1024;
const LOGO_SIZE: u32 = 96;

pub(super) fn fetch_quotes(
    assets: &[Asset],
    cached_logos: &HashMap<String, RasterImage>,
) -> Vec<MarketQuote> {
    let Ok(client) = Client::builder()
        .timeout(Duration::from_secs(9))
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
        .build()
    else {
        return assets
            .iter()
            .cloned()
            .map(|asset| failed_quote(asset, "无法创建行情连接".into(), None))
            .collect();
    };

    thread::scope(|scope| {
        let jobs = assets
            .iter()
            .cloned()
            .map(|asset| {
                let client = client.clone();
                let logo = cached_logos.get(&asset.cache_key()).cloned();
                scope.spawn(move || match asset.kind {
                    MarketKind::Stock => fetch_stock(&client, asset, logo),
                    MarketKind::Crypto => fetch_crypto(&client, asset, logo),
                })
            })
            .collect::<Vec<_>>();
        jobs.into_iter().filter_map(|job| job.join().ok()).collect()
    })
}

fn fetch_stock(client: &Client, asset: Asset, cached_logo: Option<RasterImage>) -> MarketQuote {
    let quote = fetch_stock_quote(client, &asset.symbol);
    let name = quote
        .as_ref()
        .map(|data| data.name.clone())
        .unwrap_or_else(|_| asset.symbol.clone());
    // Yahoo Finance's search endpoint no longer includes logoUrl. Resolve the
    // company name from the chart response and load its actual TradingView logo.
    let logo = cached_logo.or_else(|| stock_logo(client, &asset.symbol, &name));
    match quote {
        Ok(data) => MarketQuote {
            asset,
            name: data.name,
            price: Some(data.price),
            currency: data.currency,
            change_percent: data.change_percent,
            logo,
            error: None,
        },
        Err(error) => failed_quote(asset, error, logo),
    }
}

struct StockQuote {
    name: String,
    price: f64,
    currency: String,
    change_percent: Option<f64>,
}

fn fetch_stock_quote(client: &Client, symbol: &str) -> Result<StockQuote, String> {
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{}?range=1d&interval=1m&includePrePost=true",
        symbol
    );
    let data = get_json(client, &url)?;
    if let Some(message) = data
        .pointer("/chart/error/description")
        .and_then(Value::as_str)
    {
        return Err(message.to_owned());
    }
    let result = data
        .pointer("/chart/result/0")
        .ok_or_else(|| "找不到这个股票代码".to_owned())?;
    let meta = result
        .get("meta")
        .ok_or_else(|| "行情数据不完整".to_owned())?;
    let price = meta
        .get("regularMarketPrice")
        .and_then(Value::as_f64)
        .or_else(|| {
            result
                .pointer("/indicators/quote/0/close")
                .and_then(Value::as_array)
                .and_then(|values| values.iter().rev().find_map(Value::as_f64))
        })
        .ok_or_else(|| "当前没有可用报价".to_owned())?;
    let previous = meta
        .get("chartPreviousClose")
        .or_else(|| meta.get("previousClose"))
        .and_then(Value::as_f64);
    let change_percent = previous
        .filter(|value| *value != 0.0)
        .map(|value| (price - value) / value * 100.0);
    let name = meta
        .get("longName")
        .or_else(|| meta.get("shortName"))
        .and_then(Value::as_str)
        .unwrap_or(symbol)
        .to_owned();
    Ok(StockQuote {
        name,
        price,
        currency: meta
            .get("currency")
            .and_then(Value::as_str)
            .unwrap_or("USD")
            .to_owned(),
        change_percent,
    })
}

fn fetch_crypto(client: &Client, asset: Asset, cached_logo: Option<RasterImage>) -> MarketQuote {
    let product = format!("{}-USD", asset.symbol);
    let stats = get_json(
        client,
        &format!("https://api.exchange.coinbase.com/products/{product}/stats"),
    );
    let metadata = if cached_logo.is_none() {
        fetch_crypto_metadata(client, &asset.symbol)
    } else {
        (crypto_name(&asset.symbol), None)
    };
    let logo = cached_logo.or(metadata.1);
    match stats {
        Ok(stats) => {
            let Some(price) = stats.get("last").and_then(value_as_f64) else {
                return failed_quote(asset, "Coinbase 暂无此美元交易对".into(), logo);
            };
            let change_percent = stats
                .get("open")
                .and_then(value_as_f64)
                .filter(|value| *value != 0.0)
                .map(|value| (price - value) / value * 100.0);
            MarketQuote {
                asset,
                name: metadata.0,
                price: Some(price),
                currency: "USD".into(),
                change_percent,
                logo,
                error: None,
            }
        }
        Err(error) => failed_quote(asset, error, logo),
    }
}

fn stock_logo(client: &Client, symbol: &str, company_name: &str) -> Option<RasterImage> {
    let mut candidates = vec![known_stock_slug(symbol).map(str::to_owned)];
    let normalized = company_slug(company_name);
    if !normalized.is_empty() {
        candidates.push(Some(normalized.clone()));
        if let Some((first, _)) = normalized.split_once('-') {
            candidates.push(Some(first.to_owned()));
        }
    }
    if candidates.iter().all(Option::is_none) {
        candidates.push(Some(symbol.to_ascii_lowercase()));
    }
    let mut attempted = Vec::new();
    for slug in candidates.into_iter().flatten() {
        if attempted.contains(&slug) {
            continue;
        }
        attempted.push(slug.clone());
        let url = format!("https://s3-symbol-logo.tradingview.com/{slug}.svg");
        if let Some(logo) = download_svg(client, &url) {
            return Some(logo);
        }
    }
    None
}

fn known_stock_slug(symbol: &str) -> Option<&'static str> {
    match symbol {
        "AAPL" => Some("apple"),
        "NVDA" => Some("nvidia"),
        "TSLA" => Some("tesla"),
        "MSFT" => Some("microsoft"),
        "AMZN" => Some("amazon"),
        "GOOG" | "GOOGL" => Some("alphabet"),
        "META" => Some("meta"),
        "BRK-A" | "BRK-B" => Some("berkshire-hathaway"),
        "JPM" => Some("jpmorgan-chase"),
        "V" => Some("visa"),
        "MA" => Some("mastercard"),
        "NFLX" => Some("netflix"),
        "WMT" => Some("walmart"),
        "COST" => Some("costco"),
        "AVGO" => Some("broadcom"),
        "ORCL" => Some("oracle"),
        "CRM" => Some("salesforce"),
        "AMD" => Some("amd"),
        "INTC" => Some("intel"),
        "DIS" => Some("disney"),
        "KO" => Some("coca-cola"),
        "PEP" => Some("pepsi"),
        "TSM" => Some("tsmc"),
        _ => None,
    }
}

fn company_slug(name: &str) -> String {
    let mut words = name
        .replace(".com", "")
        .to_ascii_lowercase()
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    while words.last().is_some_and(|word| {
        matches!(
            word.as_str(),
            "inc"
                | "incorporated"
                | "corp"
                | "corporation"
                | "company"
                | "co"
                | "ltd"
                | "limited"
                | "plc"
                | "holdings"
                | "holding"
                | "class"
                | "a"
                | "b"
        )
    }) {
        words.pop();
    }
    words.join("-")
}

fn download_svg(client: &Client, url: &str) -> Option<RasterImage> {
    let bytes = bounded_bytes(client.get(url).send().ok()?)?;
    if !String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).contains("<svg") {
        return None;
    }
    let pixels = egui_extras::image::load_svg_bytes_with_size(
        &bytes,
        Some(eframe::egui::SizeHint::Size(LOGO_SIZE, LOGO_SIZE)),
    )
    .ok()?;
    let width = u16::try_from(pixels.size[0]).ok()?;
    let height = u16::try_from(pixels.size[1]).ok()?;
    let rgba = pixels
        .pixels
        .iter()
        .flat_map(|pixel| pixel.to_array())
        .collect();
    Some(RasterImage {
        size: [width, height],
        rgba,
    })
}

fn fetch_crypto_metadata(client: &Client, symbol: &str) -> (String, Option<RasterImage>) {
    let result = client
        .get("https://api.coingecko.com/api/v3/search")
        .query(&[("query", symbol)])
        .send()
        .ok()
        .and_then(|response| response.error_for_status().ok())
        .and_then(|response| response.bytes().ok())
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    let coin = result
        .as_ref()
        .and_then(|data| data.get("coins"))
        .and_then(Value::as_array)
        .and_then(|coins| {
            coins.iter().find(|coin| {
                coin.get("symbol")
                    .and_then(Value::as_str)
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(symbol))
            })
        });
    let Some(coin) = coin else {
        return (crypto_name(symbol), None);
    };
    let name = coin
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(symbol)
        .to_owned();
    let logo = coin
        .get("thumb")
        .and_then(Value::as_str)
        .and_then(|url| download_raster(client, url));
    (name, logo)
}

fn download_raster(client: &Client, url: &str) -> Option<RasterImage> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if parsed.scheme() != "https" || !(host == "coingecko.com" || host.ends_with(".coingecko.com"))
    {
        return None;
    }
    let bytes = bounded_bytes(client.get(parsed).send().ok()?)?;
    let image = image::load_from_memory(&bytes)
        .ok()?
        .thumbnail(64, 64)
        .to_rgba8();
    let size = [
        u16::try_from(image.width()).ok()?,
        u16::try_from(image.height()).ok()?,
    ];
    Some(RasterImage {
        size,
        rgba: image.into_raw(),
    })
}

fn bounded_bytes(response: Response) -> Option<Vec<u8>> {
    let response = response.error_for_status().ok()?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_LOGO_BYTES)
    {
        return None;
    }
    let bytes = response.bytes().ok()?;
    (bytes.len() as u64 <= MAX_LOGO_BYTES).then(|| bytes.to_vec())
}

fn get_json(client: &Client, url: &str) -> Result<Value, String> {
    let response = client
        .get(url)
        .send()
        .and_then(Response::error_for_status)
        .map_err(|error| friendly_request_error(&error))?;
    let bytes = response
        .bytes()
        .map_err(|_| "行情数据读取失败".to_owned())?;
    serde_json::from_slice(&bytes).map_err(|_| "行情数据格式无效".to_owned())
}

fn failed_quote(asset: Asset, error: String, logo: Option<RasterImage>) -> MarketQuote {
    let mut quote = MarketQuote::loading(asset);
    quote.logo = logo;
    quote.error = Some(error);
    quote
}

fn value_as_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse::<f64>().ok())
}

fn friendly_request_error(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        "行情连接超时".into()
    } else if error.status().is_some_and(|status| status.as_u16() == 404) {
        "找不到这个代码".into()
    } else if error.status().is_some_and(|status| status.as_u16() == 429) {
        "行情源请求过于频繁".into()
    } else {
        "暂时无法获取报价".into()
    }
}

fn crypto_name(symbol: &str) -> String {
    match symbol {
        "BTC" => "Bitcoin".into(),
        "ETH" => "Ethereum".into(),
        "SOL" => "Solana".into(),
        "XRP" => "XRP".into(),
        "BNB" => "BNB".into(),
        "DOGE" => "Dogecoin".into(),
        "ADA" => "Cardano".into(),
        "AVAX" => "Avalanche".into(),
        _ => symbol.to_owned(),
    }
}
