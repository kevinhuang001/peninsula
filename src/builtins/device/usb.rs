//! USB arrival and removal prompts. Enumeration runs off the UI thread.
use crate::{
    core::worker::Worker,
    extension::{
        ContributionDescriptor, ContributionKind, ExtensionContext, ExtensionDescriptor,
        ExtensionId, ExtensionOutput, NotificationPresentationProvider, Prompt, PromptLayer,
        RuntimeExtension,
    },
    ui::declarative::{
        ColorSpec, Easing, FontSize, FontWeight, Motion, MotionProperty, MotionTrack, TextStyle,
        ViewNode,
    },
};
#[cfg(any(target_os = "linux", test))]
use std::path::Path;
use std::{collections::BTreeMap, time::Duration};

const ID: &str = "builtin.device";
const PROMPT_ID: &str = "builtin.device/usb-prompt";
type Devices = BTreeMap<String, String>;

#[derive(Default)]
pub(super) struct UsbExtension {
    updates: Option<Worker<Option<Devices>>>,
    previous: Option<Devices>,
}

impl NotificationPresentationProvider for UsbExtension {}

impl RuntimeExtension for UsbExtension {
    fn descriptor(&self) -> ExtensionDescriptor {
        ExtensionDescriptor::builtin(
            ID,
            "USB 设备",
            "USB 设备插入与拔出提示",
            vec![ContributionDescriptor::builtin(
                PROMPT_ID,
                ContributionKind::Prompt,
                "USB 插拔提示",
                "usb",
                true,
                1,
            )],
        )
    }

    fn on_start(&mut self, ctx: &ExtensionContext) -> Result<(), String> {
        self.previous = None;
        self.updates = Some(
            Worker::spawn_scoped_latest(&ctx.tasks(), "usb", sample_interval(), sample_devices)
                .map_err(|error| error.to_string())?,
        );
        Ok(())
    }

    fn on_stop(&mut self, _: &ExtensionContext) -> Result<(), String> {
        self.updates = None;
        self.previous = None;
        Ok(())
    }

    fn update(&mut self, _: std::time::Instant) -> ExtensionOutput {
        let Some(devices) = self.updates.as_ref().and_then(Worker::take).flatten() else {
            return ExtensionOutput::default();
        };
        let prompts = self
            .previous
            .as_ref()
            .map(|previous| changes(previous, &devices))
            .unwrap_or_default();
        self.previous = Some(devices);
        ExtensionOutput {
            prompts,
            ..Default::default()
        }
    }
}

fn sample_interval() -> Duration {
    #[cfg(target_os = "macos")]
    {
        Duration::from_secs(5)
    }
    #[cfg(target_os = "windows")]
    {
        Duration::from_secs(3)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Duration::from_secs(1)
    }
}

pub(super) fn changes(previous: &Devices, current: &Devices) -> Vec<Prompt> {
    // A hub can expose several interfaces at once. Bound the queue on bursts.
    let removed = previous.iter().filter(|(id, _)| !current.contains_key(*id));
    let added = current.iter().filter(|(id, _)| !previous.contains_key(*id));
    removed
        .map(|(_, name)| usb_prompt(name, false))
        .chain(added.map(|(_, name)| usb_prompt(name, true)))
        .take(4)
        .collect()
}

fn usb_prompt(name: &str, inserted: bool) -> Prompt {
    let (verb, accent, color) = if inserted {
        ("已连接", [70, 206, 193], ColorSpec::rgb(113, 240, 220))
    } else {
        ("已移除", [239, 160, 94], ColorSpec::rgb(255, 192, 130))
    };
    let label = format!("USB {verb} · {}", short_name(name));
    let entrance = Motion {
        duration_ms: 650,
        easing: Easing::EaseOut,
        tracks: vec![
            MotionTrack::new(MotionProperty::Scale, &[(0, 0.78), (320, 1.08), (650, 1.0)]),
            MotionTrack::new(MotionProperty::Opacity, &[(0, 0.0), (180, 1.0)]),
        ],
        ..Default::default()
    };
    Prompt::new(
        ExtensionId::builtin(ID),
        ExtensionId::builtin(PROMPT_ID),
        ViewNode::text(label, TextStyle::default()),
        Duration::from_secs(3),
    )
    .with_width(230.0, 266.0)
    .with_accent(accent)
    .with_layers(vec![
        PromptLayer::new(
            [0.055, 0.06, 0.18, 0.88],
            ViewNode::text(
                "USB",
                TextStyle {
                    font_size: FontSize::Relative(0.8333),
                    font_weight: FontWeight::Bold,
                    monospace: true,
                    ..Default::default()
                },
            )
            .color(color)
            .motion(entrance),
        ),
        PromptLayer::new(
            [0.24, 0.06, 0.27, 0.88],
            ViewNode::text(
                verb,
                TextStyle {
                    font_size: FontSize::Relative(1.0),
                    font_weight: FontWeight::Bold,
                    ..Default::default()
                },
            )
            .color(color),
        ),
        PromptLayer::new(
            [0.49, 0.07, 0.46, 0.86],
            ViewNode::text(
                short_name(name),
                TextStyle {
                    font_size: FontSize::Relative(0.9167),
                    ..Default::default()
                },
            ),
        ),
    ])
}

fn short_name(name: &str) -> String {
    let clean = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let clean = if clean.is_empty() {
        "USB 设备"
    } else {
        clean.as_str()
    };
    let mut chars = clean.chars();
    let head: String = chars.by_ref().take(22).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

fn sample_devices(cancel: &crate::sdk::tasks::CancellationToken) -> Option<Devices> {
    if cancel.is_cancelled() {
        return None;
    }
    #[cfg(target_os = "linux")]
    {
        return linux_devices(Path::new("/sys/bus/usb/devices"));
    }
    #[cfg(target_os = "macos")]
    {
        return mac_devices(cancel);
    }
    #[cfg(target_os = "windows")]
    {
        return windows_devices(cancel);
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(any(target_os = "linux", test))]
fn linux_devices(root: &Path) -> Option<Devices> {
    let mut devices = Devices::new();
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let path = entry.path();
        let (Ok(vendor), Ok(product)) = (
            std::fs::read_to_string(path.join("idVendor")),
            std::fs::read_to_string(path.join("idProduct")),
        ) else {
            continue;
        };
        let class = std::fs::read_to_string(path.join("bDeviceClass")).unwrap_or_default();
        if class.trim() == "09" {
            continue;
        } // USB hubs, including root hubs
        let name = std::fs::read_to_string(path.join("product"))
            .unwrap_or_else(|_| format!("USB {}:{}", vendor.trim(), product.trim()));
        if name.to_ascii_lowercase().contains("hub") {
            continue;
        }
        devices.insert(
            entry.file_name().to_string_lossy().into_owned(),
            short_name(&name),
        );
    }
    Some(devices)
}

#[cfg(target_os = "macos")]
fn mac_devices(cancel: &crate::sdk::tasks::CancellationToken) -> Option<Devices> {
    let output = crate::sdk::process::ProcessExecutor
        .run_with(
            crate::sdk::process::ProcessSpec::new("system_profiler")
                .args(["SPUSBDataType", "-json"])
                .timeout(Duration::from_secs(8)),
            cancel,
            |_, _| {},
        )
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let mut devices = Devices::new();
    collect_mac_devices(&value, &mut devices);
    Some(devices)
}

#[cfg(any(target_os = "macos", test))]
fn collect_mac_devices(value: &serde_json::Value, devices: &mut Devices) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                collect_mac_devices(item, devices);
            }
        }
        serde_json::Value::Object(object) => {
            if let (Some(vendor), Some(product)) = (
                object.get("vendor_id").and_then(serde_json::Value::as_str),
                object.get("product_id").and_then(serde_json::Value::as_str),
            ) {
                let name = object
                    .get("_name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("USB 设备");
                if !name.to_ascii_lowercase().contains("hub") {
                    let location = object
                        .get("location_id")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or(name);
                    devices.insert(format!("{vendor}:{product}:{location}"), short_name(name));
                }
            }
            for child in object.values() {
                collect_mac_devices(child, devices);
            }
        }
        _ => {}
    }
}

#[cfg(target_os = "windows")]
fn windows_devices(cancel: &crate::sdk::tasks::CancellationToken) -> Option<Devices> {
    let script = r#"[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like 'USB\VID_*' -and $_.InstanceId -notlike '*&MI_*' -and $_.FriendlyName -notmatch 'Hub|Controller' } | Select-Object InstanceId,FriendlyName | ConvertTo-Json -Compress"#;
    let output = crate::sdk::process::ProcessExecutor
        .run_with(
            crate::sdk::process::ProcessSpec::new("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-Command", script])
                .timeout(Duration::from_secs(6)),
            cancel,
            |_, _| {},
        )
        .ok()?;
    if !output.status.success() {
        return None;
    }
    if output.stdout.is_empty() {
        return Some(Devices::new());
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let mut devices = Devices::new();
    let items = value
        .as_array()
        .map_or_else(|| vec![&value], |items| items.iter().collect());
    for item in items {
        if let Some(id) = item.get("InstanceId").and_then(serde_json::Value::as_str) {
            let name = item
                .get("FriendlyName")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("USB 设备");
            devices.insert(id.to_owned(), short_name(name));
        }
    }
    Some(devices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insertion_and_removal_emit_only_after_baseline() {
        let old = Devices::from([("1-1".into(), "键盘".into())]);
        let new = Devices::from([("2-1".into(), "U 盘".into())]);
        let prompts = changes(&old, &new);
        assert_eq!(prompts.len(), 2);
        assert_eq!(prompts[0].accent, Some([239, 160, 94]));
        assert_eq!(prompts[1].accent, Some([70, 206, 193]));
        assert!(changes(&new, &new).is_empty());
        assert_eq!(prompts[0].layers.len(), 3);
    }

    #[test]
    fn first_sample_sets_baseline_without_startup_prompts() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut extension = UsbExtension {
            updates: Some(Worker::from_receiver(rx)),
            previous: None,
        };
        tx.send(Some(Devices::from([("1-1".into(), "键盘".into())])))
            .unwrap();
        assert!(extension
            .update(std::time::Instant::now())
            .prompts
            .is_empty());
        tx.send(None).unwrap();
        assert!(extension
            .update(std::time::Instant::now())
            .prompts
            .is_empty());
        tx.send(Some(Devices::new())).unwrap();
        assert_eq!(extension.update(std::time::Instant::now()).prompts.len(), 1);
    }

    #[test]
    fn linux_probe_skips_hubs_and_reads_product_name() {
        let root = tempfile::tempdir().unwrap();
        for (id, class, name) in [("1-0", "09", "Root Hub"), ("1-1", "00", "USB Disk")] {
            let path = root.path().join(id);
            std::fs::create_dir(&path).unwrap();
            std::fs::write(path.join("idVendor"), "1234").unwrap();
            std::fs::write(path.join("idProduct"), "5678").unwrap();
            std::fs::write(path.join("bDeviceClass"), class).unwrap();
            std::fs::write(path.join("product"), name).unwrap();
        }
        let devices = linux_devices(root.path()).unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices["1-1"], "USB Disk");
    }

    #[test]
    fn mac_probe_finds_nested_devices_without_hubs() {
        let tree = serde_json::json!({"SPUSBDataType": [{"_name": "Controller", "_items": [
            {"_name": "USB Hub", "vendor_id": "0x1234", "product_id": "0x0001", "location_id": "0x1"},
            {"_name": "Camera", "vendor_id": "0x1234", "product_id": "0x0002", "location_id": "0x2"}
        ]}]});
        let mut devices = Devices::new();
        collect_mac_devices(&tree, &mut devices);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices["0x1234:0x0002:0x2"], "Camera");
    }
}
