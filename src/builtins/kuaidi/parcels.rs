use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::SystemTime};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Parcel {
    pub order_id: String,
    #[serde(default)]
    pub order_time: String,
    #[serde(default)]
    pub product_name: String,
    #[serde(default)]
    pub carrier: String,
    #[serde(default)]
    pub tracking_no: String,
    #[serde(default)]
    pub current_status: String,
    #[serde(default)]
    pub latest_time: String,
    #[serde(default)]
    pub latest_detail: String,
}

impl Parcel {
    pub fn logistics(&self) -> bool {
        !self.current_status.is_empty() || !self.latest_detail.is_empty()
    }

    pub fn signed(&self) -> bool {
        matches!(
            self.current_status.as_str(),
            "已签收" | "已收货" | "已取件" | "已妥投" | "妥投" | "订单完成"
        )
    }

    pub fn awaiting_pickup(&self) -> bool {
        !self.signed()
            && (matches!(
                self.current_status.trim(),
                "待取件"
                    | "等待取件"
                    | "可取件"
                    | "待自提"
                    | "等待自提"
                    | "待客户自提"
                    | "待领取"
                    | "等待领取"
            ) || ["请凭取件码", "请尽快取件"]
                .iter()
                .any(|phrase| self.latest_detail.contains(phrase)))
    }

    fn event_changed(&self, previous: &Self) -> bool {
        (!self.current_status.is_empty() && self.current_status != previous.current_status)
            || (!self.latest_time.is_empty() && self.latest_time != previous.latest_time)
            || (!self.latest_detail.is_empty() && self.latest_detail != previous.latest_detail)
            || (!self.carrier.is_empty() && self.carrier != previous.carrier)
            || (!self.tracking_no.is_empty() && self.tracking_no != previous.tracking_no)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ShipmentChange {
    Updated(usize),
    ReadyForPickup(usize),
}

impl ShipmentChange {
    fn between(index: usize, parcel: &Parcel, previous: Option<&Parcel>) -> Self {
        if parcel.awaiting_pickup() && !previous.is_some_and(Parcel::awaiting_pickup) {
            Self::ReadyForPickup(index)
        } else {
            Self::Updated(index)
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Snapshot {
    pub checked_at: SystemTime,
    pub parcels: Vec<Parcel>,
}

impl Snapshot {
    pub fn changes(&mut self, previous: Option<&Self>) -> Vec<ShipmentChange> {
        let Some(previous) = previous else {
            return vec![];
        };
        let mut by_order: HashMap<&str, Vec<&Parcel>> = HashMap::new();
        for parcel in &previous.parcels {
            by_order.entry(&parcel.order_id).or_default().push(parcel);
        }
        let mut changes = Vec::new();
        for (index, parcel) in self.parcels.iter_mut().enumerate() {
            let Some(same_order) = by_order.get(parcel.order_id.as_str()) else {
                if parcel.logistics() {
                    changes.push(ShipmentChange::between(index, parcel, None));
                }
                continue;
            };
            let prior = same_order
                .iter()
                .copied()
                .find(|old| !parcel.tracking_no.is_empty() && parcel.tracking_no == old.tracking_no)
                .or(match same_order.as_slice() {
                    [old] if parcel.tracking_no.is_empty() || old.tracking_no.is_empty() => {
                        Some(*old)
                    }
                    _ => None,
                });
            let Some(prior) = prior else {
                if !parcel.tracking_no.is_empty() && parcel.logistics() {
                    changes.push(ShipmentChange::between(index, parcel, None));
                }
                continue;
            };

            // The plugin can fall back to an order summary when a detail page fails.
            // A status-only summary is weaker than a previously observed node.
            if !parcel.signed()
                && !parcel.awaiting_pickup()
                && parcel.tracking_no.is_empty()
                && parcel.latest_time.is_empty()
                && parcel.latest_detail.is_empty()
                && !prior.latest_time.is_empty()
            {
                parcel.current_status.clone_from(&prior.current_status);
                parcel.latest_time.clone_from(&prior.latest_time);
                parcel.latest_detail.clone_from(&prior.latest_detail);
            }
            let changed = parcel.event_changed(prior);
            if parcel.tracking_no.is_empty() {
                parcel.tracking_no.clone_from(&prior.tracking_no);
            }
            if parcel.carrier.is_empty() {
                parcel.carrier.clone_from(&prior.carrier);
            }
            if parcel.current_status.is_empty() {
                parcel.current_status.clone_from(&prior.current_status);
            }
            if parcel.latest_detail.is_empty() && parcel.latest_time == prior.latest_time {
                parcel.latest_detail.clone_from(&prior.latest_detail);
            }
            if changed {
                changes.push(ShipmentChange::between(index, parcel, Some(prior)));
            }
        }
        changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk::storage::{AreaKind, StorageArea};

    fn parcel(status: &str, time: &str, detail: &str) -> Parcel {
        Parcel {
            order_id: "001".into(),
            order_time: String::new(),
            product_name: "数据线".into(),
            carrier: "快递".into(),
            tracking_no: "ABC".into(),
            current_status: status.into(),
            latest_time: time.into(),
            latest_detail: detail.into(),
        }
    }

    fn snapshot(parcels: Vec<Parcel>) -> Snapshot {
        Snapshot {
            checked_at: SystemTime::now(),
            parcels,
        }
    }

    #[test]
    fn compares_saved_logistics_and_detects_a_new_node_in_the_same_status() {
        let dir = tempfile::tempdir().unwrap();
        let storage = StorageArea::from_root(AreaKind::Data, dir.path());
        let mut first = snapshot(vec![parcel("运输中", "10:00", "已离开网点")]);
        assert!(first.changes(None).is_empty());
        storage.write_json("taobao.json", &first).unwrap();
        let previous: Snapshot = storage.read_json("taobao.json").unwrap().unwrap();
        assert!(first.changes(Some(&previous)).is_empty());
        let mut renamed = first.clone();
        renamed.parcels[0].product_name = "新名称".into();
        assert!(renamed.changes(Some(&previous)).is_empty());
        let mut new_node = snapshot(vec![parcel("运输中", "11:00", "已到达分拨中心")]);
        assert_eq!(
            new_node.changes(Some(&previous)),
            vec![ShipmentChange::Updated(0)]
        );
        let mut corrected_detail = snapshot(vec![parcel("运输中", "11:00", "已到达中转站")]);
        assert_eq!(
            corrected_detail.changes(Some(&new_node)),
            vec![ShipmentChange::Updated(0)]
        );
        let mut signed = snapshot(vec![parcel("已签收", "", "已取件")]);
        assert_eq!(
            signed.changes(Some(&previous)),
            vec![ShipmentChange::Updated(0)]
        );
    }

    #[test]
    fn handles_new_shipments_and_never_assumes_a_disappearing_order_was_delivered() {
        let mut before = snapshot(vec![parcel("运输中", "10:00", "已离开网点")]);
        let mut fallback = snapshot(vec![parcel("卖家已发货", "", "")]);
        fallback.parcels[0].tracking_no.clear();
        assert!(fallback.changes(Some(&before)).is_empty());
        assert_eq!(fallback.parcels[0].current_status, "运输中");
        assert_eq!(fallback.parcels[0].tracking_no, "ABC");

        let mut empty = snapshot(vec![]);
        assert!(empty.changes(Some(&before)).is_empty());
        before.parcels[0].order_id = "002".into();
        assert_eq!(
            before.changes(Some(&empty)),
            vec![ShipmentChange::Updated(0)]
        );
    }

    #[test]
    fn matches_two_packages_in_one_order_by_tracking_number() {
        let first = parcel("运输中", "10:00", "已离开网点");
        let mut second = first.clone();
        second.tracking_no = "DEF".into();
        second.current_status = "待取件".into();
        let previous = snapshot(vec![first.clone(), second.clone()]);
        second.current_status = "已签收".into();
        let mut updated = snapshot(vec![second, first]);
        assert_eq!(
            updated.changes(Some(&previous)),
            vec![ShipmentChange::Updated(0)]
        );
    }
}
