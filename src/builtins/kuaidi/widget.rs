use super::{
    parcels::Parcel,
    poller::{Platform, SyncError, PLUGIN_INSTALL},
    Source,
};
use crate::ui::declarative::*;

pub(super) fn build(sources: &[Source; 2]) -> WidgetView {
    let mut parcels: Vec<_> = sources
        .iter()
        .filter_map(|source| source.snapshot.as_ref())
        .flat_map(|snapshot| &snapshot.parcels)
        .collect();
    parcels.sort_by_key(|parcel| {
        std::cmp::Reverse(if parcel.latest_time.is_empty() {
            parcel.order_time.as_str()
        } else {
            parcel.latest_time.as_str()
        })
    });

    let mut rows = Vec::new();
    for platform in Platform::ALL {
        let source = &sources[platform as usize];
        if let Some(error) = source.error {
            let mut message = match error {
                SyncError::OpenCli => "未能运行 OpenCLI，请检查安装。".into(),
                SyncError::Plugin => format!("请安装插件：{PLUGIN_INSTALL}"),
                SyncError::Login => format!("请在 Chrome 登录{}。", platform.name()),
                SyncError::Browser => "请打开 Chrome 并连接 OpenCLI Browser Bridge。".into(),
                SyncError::Query => format!(
                    "{}查询失败，请检查 Chrome 登录与 OpenCLI 连接。",
                    platform.name()
                ),
                SyncError::Save => "结果已更新，未能保存到本地。".into(),
            };
            if source.snapshot.is_some() && error != SyncError::Save {
                message.push_str("显示上次结果。");
            }
            if !rows.iter().any(|(existing, _)| *existing == message) {
                let node = cell(&message, ColorSpec::Warning, true);
                rows.push((message, node));
            }
        }
    }
    let mut content: Vec<_> = rows.into_iter().map(|(_, node)| node).collect();
    if parcels.is_empty() {
        if content.is_empty() {
            let label = if sources.iter().all(|source| source.snapshot.is_some()) {
                "暂无待跟踪快递"
            } else {
                "等待首次查询…"
            };
            content.push(cell(label, ColorSpec::MutedText, false));
        }
    } else {
        let columns = [
            ("商品", 1.65),
            ("快递公司", 0.80),
            ("运单号", 1.24),
            ("状态", 0.70),
            ("更新时间", 1.24),
            ("最新物流", 2.05),
        ]
        .into_iter()
        .map(|(title, width)| TableColumn::new(title, width))
        .collect();
        let table = Table {
            columns,
            rows: parcels.into_iter().map(parcel_row).collect(),
        };
        content.push(ViewNode::new(Node::Table(table)));
    }
    let root = ViewNode::column(
        content,
        Flow {
            gap: Some(6.0),
            align: Align::Start,
            ..Flow::default()
        },
    )
    .sized(Size::Fill(1.0), Size::Auto);
    WidgetView::new(root, [0.0, 0.0])
}

fn parcel_row(parcel: &Parcel) -> Vec<ViewNode> {
    let title = if parcel.product_name.is_empty() {
        "订单快递"
    } else {
        &parcel.product_name
    };
    let status = if parcel.current_status.is_empty() {
        "等待更新"
    } else {
        &parcel.current_status
    };
    let status_color = if parcel.signed() {
        ColorSpec::Success
    } else if status.contains("异常") || status.contains("退回") {
        ColorSpec::Danger
    } else if parcel.awaiting_pickup() {
        ColorSpec::Warning
    } else {
        ColorSpec::Info
    };
    vec![
        cell(title, ColorSpec::PrimaryText, true),
        cell(&parcel.carrier, ColorSpec::SecondaryText, true),
        cell(&parcel.tracking_no, ColorSpec::SecondaryText, true),
        cell(status, status_color, true),
        cell(&parcel.latest_time, ColorSpec::SecondaryText, true),
        cell(&parcel.latest_detail, ColorSpec::SecondaryText, true),
    ]
}

fn cell(value: &str, color: ColorSpec, wrap: bool) -> ViewNode {
    ViewNode::text(
        if value.is_empty() { "—" } else { value },
        TextStyle {
            align: Align::Start,
            font_size: FontSize::Custom(10.5),
            wrap,
            ..TextStyle::default()
        },
    )
    .color(color)
}
