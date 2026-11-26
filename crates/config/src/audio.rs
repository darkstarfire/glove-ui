//! Sound outputs and inputs from PipeWire, by port and not only by sink.
//!
//! `wpctl status` lists only the sinks of the profile the card is using. Many laptop
//! cards (the SOF/UCM ones) put "Speaker" and "Headphones" in different profiles, so a
//! headset plugged into the jack never shows up there. Here every port of every card
//! (`EnumRoute` in `pw-dump`) is a choice; choosing one switches the card profile when
//! it has to, the port when the sink has more than one, and makes it the default.

use std::process::Command;
use std::time::{Duration, Instant};

use glove_i18n::tr;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Output,
    Input,
}

impl Direction {
    fn media_class(self) -> &'static str {
        match self {
            Direction::Output => "Audio/Sink",
            Direction::Input => "Audio/Source",
        }
    }
    fn route_dir(self) -> &'static str {
        match self {
            Direction::Output => "Output",
            Direction::Input => "Input",
        }
    }
    fn default_key(self) -> &'static str {
        match self {
            Direction::Output => "default.audio.sink",
            Direction::Input => "default.audio.source",
        }
    }
}

/// What kind of device a port is (the "Type" column and the icon).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Speakers or microphone of the computer.
    Builtin,
    /// Headphones or headset in the jack of the computer.
    Headphones,
    Hdmi,
    Usb,
    Bluetooth,
    /// A sink or source made by software (filters, virtual devices).
    Virtual,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Builtin | Kind::Headphones => tr!("Interno"),
            Kind::Hdmi => "HDMI",
            Kind::Usb => "USB",
            Kind::Bluetooth => "Bluetooth",
            Kind::Virtual => tr!("Virtual"),
        }
    }
}

/// One output or input the user can choose.
#[derive(Clone, Debug, PartialEq)]
pub struct Port {
    pub name: String,
    pub kind: Kind,
    /// The PipeWire device (card) and the port in it; None for a software sink.
    pub device: Option<u32>,
    pub route: Option<u32>,
    /// The sub-device of the card that plays this port (`card.profile.device`).
    pub route_device: Option<i64>,
    /// The card profiles that have this port.
    pub profiles: Vec<u32>,
    /// The sink or source playing this port right now (None if its profile or port is
    /// not the one in use).
    pub node: Option<u32>,
    pub default: bool,
    /// Volume as `wpctl` shows it (cubic), and mute; only while `node` is Some.
    pub volume: f32,
    pub muted: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub outputs: Vec<Port>,
    pub inputs: Vec<Port>,
}

impl Snapshot {
    pub fn ports(&self, dir: Direction) -> &[Port] {
        match dir {
            Direction::Output => &self.outputs,
            Direction::Input => &self.inputs,
        }
    }
}

fn props(o: &Value) -> Option<&serde_json::Map<String, Value>> {
    o.get("info")?.get("props")?.as_object()
}

fn params<'a>(o: &'a Value, key: &str) -> &'a [Value] {
    o.get("info").and_then(|i| i.get("params")).and_then(|p| p.get(key)).and_then(|v| v.as_array()).map_or(&[], |v| v.as_slice())
}

fn prop_str<'a>(o: &'a Value, key: &str) -> Option<&'a str> {
    props(o)?.get(key)?.as_str()
}

/// A number in props: PipeWire writes ids sometimes as numbers, sometimes as strings.
fn prop_num(o: &Value, key: &str) -> Option<i64> {
    let v = props(o)?.get(key)?;
    v.as_i64().or_else(|| v.as_str()?.parse().ok())
}

fn is_type(o: &Value, t: &str) -> bool {
    o.get("type").and_then(|v| v.as_str()).is_some_and(|s| s.ends_with(t))
}

/// `info` of a route: a flat list [count, key, value, key, value, ...].
fn route_info<'a>(r: &'a Value, key: &str) -> Option<&'a str> {
    let a = r.get("info")?.as_array()?;
    a.iter().skip(1).collect::<Vec<_>>().chunks(2).find(|kv| kv[0].as_str() == Some(key)).and_then(|kv| kv.get(1)?.as_str())
}

fn u32s(v: Option<&Value>) -> Vec<u32> {
    v.and_then(|v| v.as_array()).map_or(Vec::new(), |a| a.iter().filter_map(|x| x.as_u64().map(|n| n as u32)).collect())
}

/// Port names the ALSA/UCM profiles use, in the language of the system.
fn port_name(desc: &str, port_type: &str) -> String {
    match port_type {
        "speaker" => return tr!("Alto-falantes internos").to_string(),
        "headphones" => return tr!("Fones de ouvido").to_string(),
        "headset" => return tr!("Headset").to_string(),
        _ => {}
    }
    match desc {
        "Speaker" | "Speakers" => tr!("Alto-falantes internos").to_string(),
        "Headphones" => tr!("Fones de ouvido").to_string(),
        "Headset" | "Headset Microphone" => tr!("Microfone do headset").to_string(),
        "Digital Microphone" | "Internal Microphone" => tr!("Microfone interno").to_string(),
        "Stereo Microphone" | "Microphone" => tr!("Microfone").to_string(),
        "Line Out" => tr!("Saída de linha").to_string(),
        "Line In" => tr!("Entrada de linha").to_string(),
        d => d.trim_end_matches(" Output").trim_end_matches(" Input").to_string(),
    }
}

/// Volume of a node: the loudest channel, cubic like `wpctl`; and mute.
fn node_volume(n: &Value) -> (f32, bool) {
    for p in params(n, "Props") {
        if let Some(ch) = p.get("channelVolumes").and_then(|v| v.as_array()) {
            let lin = ch.iter().filter_map(|x| x.as_f64()).fold(0.0f64, f64::max);
            let muted = p.get("mute").and_then(|v| v.as_bool()).unwrap_or(false);
            return ((lin.cbrt() as f32).clamp(0.0, 1.5), muted);
        }
    }
    (1.0, false)
}

/// The default sink or source name from the "default" metadata.
fn default_name(dump: &[Value], dir: Direction) -> Option<String> {
    let meta = dump.iter().find(|o| is_type(o, "Metadata") && o.get("props").and_then(|p| p.get("metadata.name")).and_then(|v| v.as_str()) == Some("default"))?;
    let entry = meta.get("metadata")?.as_array()?.iter().find(|m| m.get("key").and_then(|k| k.as_str()) == Some(dir.default_key()))?;
    let v = entry.get("value")?;
    // A JSON object, or the same JSON as a string in older PipeWire.
    let obj = if let Some(s) = v.as_str() { serde_json::from_str::<Value>(s).ok()? } else { v.clone() };
    obj.get("name")?.as_str().map(str::to_string)
}

/// The choices of one direction, from the JSON of `pw-dump`.
fn ports_of(dump: &[Value], dir: Direction) -> Vec<Port> {
    let default = default_name(dump, dir);
    let nodes: Vec<&Value> = dump.iter().filter(|o| is_type(o, "Node") && prop_str(o, "media.class") == Some(dir.media_class())).collect();
    let mut used = vec![false; nodes.len()];
    let mut out = Vec::new();
    // Cards that list their ports: their sinks are shown by port (a sink of a port with
    // nothing plugged, like an HDMI without a monitor, is not shown at all).
    let mut by_port: Vec<i64> = Vec::new();
    for dev in dump.iter().filter(|o| is_type(o, "Device") && prop_str(o, "media.class") == Some("Audio/Device")) {
        let Some(dev_id) = dev.get("id").and_then(|v| v.as_u64()).map(|n| n as u32) else { continue };
        let bus = prop_str(dev, "device.bus").unwrap_or("");
        let api = prop_str(dev, "device.api").unwrap_or("");
        let dev_name = prop_str(dev, "device.description").or_else(|| prop_str(dev, "device.nick")).unwrap_or("").to_string();
        // Which ports are playing comes from "Route" and the sinks that exist, never from
        // "Profile": after a profile switch some cards (SOF/UCM) keep reporting the old one.
        let active: Vec<(u32, i64)> = params(dev, "Route")
            .iter()
            .filter_map(|r| Some((r.get("index")?.as_u64()? as u32, r.get("device")?.as_i64()?)))
            .collect();
        if !params(dev, "EnumRoute").is_empty() {
            by_port.push(dev_id as i64);
        }
        let routes: Vec<&Value> = params(dev, "EnumRoute")
            .iter()
            .filter(|r| r.get("direction").and_then(|v| v.as_str()) == Some(dir.route_dir()))
            .filter(|r| r.get("available").and_then(|v| v.as_str()) != Some("no"))
            .collect();
        let many = routes.len() > 1;
        for r in routes {
            let Some(index) = r.get("index").and_then(|v| v.as_u64()).map(|n| n as u32) else { continue };
            let profiles = u32s(r.get("profiles"));
            if profiles.is_empty() {
                continue;
            }
            let desc = r.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let port_type = route_info(r, "port.type").unwrap_or("");
            let kind = if bus == "bluetooth" || api == "bluez5" {
                Kind::Bluetooth
            } else if bus == "usb" {
                Kind::Usb
            } else if port_type == "hdmi" || desc.contains("HDMI") || desc.contains("DisplayPort") {
                Kind::Hdmi
            } else if matches!(port_type, "headphones" | "headset") || desc.contains("Headphone") || desc.contains("Headset") {
                Kind::Headphones
            } else {
                Kind::Builtin
            };
            let name = match kind {
                // A plugged monitor tells its name.
                Kind::Hdmi => match route_info(r, "device.product.name") {
                    Some(p) if !p.is_empty() => p.to_string(),
                    _ => port_name(desc, port_type),
                },
                Kind::Usb | Kind::Bluetooth if !dev_name.is_empty() => {
                    if many {
                        format!("{dev_name} – {}", port_name(desc, port_type))
                    } else {
                        dev_name.clone()
                    }
                }
                _ => port_name(desc, port_type),
            };
            // The sub-device of the card for this port.
            let route_device = r.get("devices").and_then(|v| v.as_array()).and_then(|a| a.first()).and_then(|v| v.as_i64());
            let live = route_device.is_some_and(|d| active.iter().any(|(i, ad)| *i == index && *ad == d));
            let mut port = Port { name, kind, device: Some(dev_id), route: Some(index), route_device, profiles, node: None, default: false, volume: 1.0, muted: false };
            if let Some(k) = nodes.iter().position(|n| prop_num(n, "device.id") == Some(dev_id as i64) && prop_num(n, "card.profile.device") == route_device) {
                // A sink playing another port of the same sub-device is marked as used too,
                // so it does not show up again below as a nameless sink.
                used[k] = true;
                if live {
                    let n = nodes[k];
                    port.node = n.get("id").and_then(|v| v.as_u64()).map(|v| v as u32);
                    port.default = default.is_some() && prop_str(n, "node.name") == default.as_deref();
                    (port.volume, port.muted) = node_volume(n);
                }
            }
            out.push(port);
        }
    }
    // Sinks and sources without a card port: software ones, or cards without routes.
    for (k, n) in nodes.iter().enumerate() {
        if used[k] || prop_num(n, "device.id").is_some_and(|d| by_port.contains(&d)) {
            continue;
        }
        // Monitors of sinks are sources too; skip them.
        if prop_str(n, "node.name").is_some_and(|s| s.ends_with(".monitor")) {
            continue;
        }
        let name = prop_str(n, "node.description").or_else(|| prop_str(n, "node.nick")).or_else(|| prop_str(n, "node.name")).unwrap_or("?").to_string();
        let kind = if prop_num(n, "device.id").is_some() { Kind::Builtin } else { Kind::Virtual };
        let (volume, muted) = node_volume(n);
        let default = default.is_some() && prop_str(n, "node.name") == default.as_deref();
        out.push(Port { name, kind, device: None, route: None, route_device: None, profiles: Vec::new(), node: n.get("id").and_then(|v| v.as_u64()).map(|v| v as u32), default, volume, muted });
    }
    // Same names (two HDMI outputs of one card with nothing plugged...) get a number.
    let names: Vec<String> = out.iter().map(|p| p.name.clone()).collect();
    let mut seen = std::collections::HashMap::<String, u32>::new();
    for p in out.iter_mut() {
        if names.iter().filter(|n| **n == p.name).count() > 1 {
            let k = seen.entry(p.name.clone()).or_insert(0);
            *k += 1;
            p.name = format!("{} ({})", p.name, k);
        }
    }
    out
}

pub fn parse_dump(json: &str) -> Option<Snapshot> {
    let v: Value = serde_json::from_str(json).ok()?;
    let dump = v.as_array()?;
    Some(Snapshot { outputs: ports_of(dump, Direction::Output), inputs: ports_of(dump, Direction::Input) })
}

fn dump_json() -> Option<Vec<Value>> {
    let out = crate::comando::output_within(&mut Command::new("pw-dump"), Duration::from_secs(3))?;
    if !out.status.success() {
        return None;
    }
    serde_json::from_slice::<Value>(&out.stdout).ok()?.as_array().cloned()
}

/// The outputs and inputs now (runs `pw-dump`: call it outside the main thread).
/// None without PipeWire.
pub fn snapshot() -> Option<Snapshot> {
    let dump = dump_json()?;
    Some(Snapshot { outputs: ports_of(&dump, Direction::Output), inputs: ports_of(&dump, Direction::Input) })
}

fn wpctl(args: &[&str]) -> bool {
    crate::comando::output_within(Command::new("wpctl").args(args), Duration::from_secs(3)).is_some_and(|o| o.status.success())
}

/// The profile to switch to for `port`: the available one with the highest priority
/// that has it.
fn best_profile(dev: &Value, port: &Port) -> Option<u32> {
    params(dev, "EnumProfile")
        .iter()
        .filter_map(|p| {
            let i = p.get("index")?.as_u64()? as u32;
            let ok = port.profiles.contains(&i) && p.get("available").and_then(|v| v.as_str()) != Some("no");
            ok.then(|| (p.get("priority").and_then(|v| v.as_i64()).unwrap_or(0), i))
        })
        .max()
        .map(|(_, i)| i)
}

/// Makes `port` the default output or input: switches the card profile and the port
/// when needed and waits for its sink or source to appear. Blocks for up to a few
/// seconds (call it outside the main thread). False if it did not work.
pub fn select(port: &Port, dir: Direction) -> bool {
    let (Some(dev_id), Some(route)) = (port.device, port.route) else {
        return port.node.is_some_and(|n| wpctl(&["set-default", &n.to_string()]));
    };
    if let Some(n) = port.node {
        return wpctl(&["set-default", &n.to_string()]);
    }
    let Some(dump) = dump_json() else { return false };
    let Some(dev) = dump.iter().find(|o| is_type(o, "Device") && o.get("id").and_then(|v| v.as_u64()) == Some(dev_id as u64)) else { return false };
    // A sink of the same sub-device already exists: only its port changes (below).
    // Otherwise the profile in use does not have the port: switch to one that has it.
    let sub_sink = dump.iter().any(|n| is_type(n, "Node") && prop_str(n, "media.class") == Some(dir.media_class()) && prop_num(n, "device.id") == Some(dev_id as i64) && prop_num(n, "card.profile.device") == port.route_device);
    if !sub_sink {
        let Some(profile) = best_profile(dev, port) else { return false };
        if !wpctl(&["set-profile", &dev_id.to_string(), &profile.to_string()]) {
            return false;
        }
    }
    // Wait for the sink or source of the port; switch the port of that sink if it is
    // playing another one.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut route_set = false;
    while Instant::now() < deadline {
        if let Some(dump) = dump_json() {
            let ports = ports_of(&dump, dir);
            if let Some(p) = ports.iter().find(|p| p.device == Some(dev_id) && p.route == Some(route)) {
                if let Some(n) = p.node {
                    return wpctl(&["set-default", &n.to_string()]);
                }
            }
            if !route_set {
                // The sink of the same sub-device exists: it is on another port.
                let node = dump.iter().find(|n| is_type(n, "Node") && prop_str(n, "media.class") == Some(dir.media_class()) && prop_num(n, "device.id") == Some(dev_id as i64) && prop_num(n, "card.profile.device") == port.route_device);
                if let Some(id) = node.and_then(|n| n.get("id")).and_then(|v| v.as_u64()) {
                    route_set = true;
                    wpctl(&["set-route", &id.to_string(), &route.to_string()]);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A laptop card with Speaker and Headphones in different profiles (as SOF/UCM
    /// does), a Bluetooth headset and a software sink.
    const DUMP: &str = r#"[
      {"id": 48, "type": "PipeWire:Interface:Device", "info": {
        "props": {"media.class": "Audio/Device", "device.api": "alsa", "device.bus": "pci", "device.description": "Built-in Audio"},
        "params": {
          "Profile": [{"index": 2}],
          "EnumProfile": [
            {"index": 0, "name": "off", "priority": 0, "available": "yes"},
            {"index": 1, "name": "HiFi (HDMI1, Headphones, Mic1)", "priority": 10300, "available": "yes"},
            {"index": 2, "name": "HiFi (HDMI1, Mic1, Speaker)", "priority": 10200, "available": "yes"}],
          "Route": [
            {"index": 2, "direction": "Output", "device": 2, "profiles": [2, 1]},
            {"index": 3, "direction": "Output", "device": 3, "profiles": [2]},
            {"index": 5, "direction": "Input", "device": 5, "profiles": [2, 1]}],
          "EnumRoute": [
            {"index": 1, "name": "[Out] HDMI2", "description": "HDMI / DisplayPort 2 Output", "available": "no", "direction": "Output", "profiles": [2, 1], "devices": [1], "info": [2, "port.type", "hdmi"]},
            {"index": 2, "name": "[Out] HDMI1", "description": "HDMI / DisplayPort 1 Output", "available": "yes", "direction": "Output", "profiles": [2, 1], "devices": [2], "info": [2, "port.type", "hdmi", "device.product.name", "DELL U2717D"]},
            {"index": 3, "name": "[Out] Speaker", "description": "Speaker", "available": "unknown", "direction": "Output", "profiles": [2], "devices": [3], "info": [1, "port.type", "speaker"]},
            {"index": 5, "name": "[In] Mic1", "description": "Digital Microphone", "available": "unknown", "direction": "Input", "profiles": [2, 1], "devices": [5], "info": [1, "port.type", "mic"]},
            {"index": 6, "name": "[Out] Headphones", "description": "Headphones", "available": "yes", "direction": "Output", "profiles": [1], "devices": [6], "info": [1, "port.type", "headphones"]}]
        }}},
      {"id": 53, "type": "PipeWire:Interface:Node", "info": {"props": {"media.class": "Audio/Sink", "node.name": "alsa_output.hdmi2", "node.description": "Built-in Audio HDMI 2", "device.id": 48, "card.profile.device": 1}, "params": {}}},
      {"id": 52, "type": "PipeWire:Interface:Node", "info": {"props": {"media.class": "Audio/Sink", "node.name": "alsa_output.hdmi1", "device.id": 48, "card.profile.device": 2}, "params": {"Props": [{"channelVolumes": [1.0, 1.0], "mute": false}]}}},
      {"id": 55, "type": "PipeWire:Interface:Node", "info": {"props": {"media.class": "Audio/Sink", "node.name": "alsa_output.speaker", "device.id": 48, "card.profile.device": 3}, "params": {"Props": [{"channelVolumes": [0.009261, 0.009261], "mute": true}]}}},
      {"id": 57, "type": "PipeWire:Interface:Node", "info": {"props": {"media.class": "Audio/Source", "node.name": "alsa_input.mic1", "device.id": "48", "card.profile.device": "5"}, "params": {"Props": [{"channelVolumes": [1.0]}]}}},
      {"id": 70, "type": "PipeWire:Interface:Device", "info": {
        "props": {"media.class": "Audio/Device", "device.api": "bluez5", "device.bus": "bluetooth", "device.description": "JBL Tune 510BT"},
        "params": {
          "Profile": [{"index": 1}],
          "EnumProfile": [{"index": 0, "priority": 0}, {"index": 1, "priority": 40, "available": "yes"}],
          "Route": [{"index": 0, "direction": "Output", "device": 0, "profiles": [1]}],
          "EnumRoute": [{"index": 0, "description": "Headphone", "available": "yes", "direction": "Output", "profiles": [1], "devices": [0], "info": [1, "port.type", "headphone"]}]
        }}},
      {"id": 71, "type": "PipeWire:Interface:Node", "info": {"props": {"media.class": "Audio/Sink", "node.name": "bluez_output.jbl", "device.id": 70, "card.profile.device": 0}, "params": {"Props": [{"channelVolumes": [0.125, 0.125]}]}}},
      {"id": 90, "type": "PipeWire:Interface:Node", "info": {"props": {"media.class": "Audio/Sink", "node.name": "effect_input.eq", "node.description": "Equalizer"}, "params": {}}},
      {"id": 30, "type": "PipeWire:Interface:Metadata", "props": {"metadata.name": "default"}, "metadata": [
        {"subject": 0, "key": "default.audio.sink", "type": "Spa:String:JSON", "value": {"name": "alsa_output.speaker"}},
        {"subject": 0, "key": "default.audio.source", "type": "Spa:String:JSON", "value": "{\"name\": \"alsa_input.mic1\"}"}]}
    ]"#;

    #[test]
    fn lists_ports_of_other_profiles() {
        let s = parse_dump(DUMP).unwrap();
        let names: Vec<&str> = s.outputs.iter().map(|p| p.name.as_str()).collect();
        // HDMI2 is unplugged; Headphones is in the other profile but shows up.
        assert_eq!(names, ["DELL U2717D", "Alto-falantes internos", "Fones de ouvido", "JBL Tune 510BT", "Equalizer"]);
        let speaker = &s.outputs[1];
        assert_eq!((speaker.node, speaker.default, speaker.muted, speaker.kind), (Some(55), true, true, Kind::Builtin));
        assert!((speaker.volume - 0.21).abs() < 0.001, "{}", speaker.volume);
        let phones = &s.outputs[2];
        assert_eq!((phones.node, phones.default, phones.kind, phones.profiles.clone(), phones.route), (None, false, Kind::Headphones, vec![1], Some(6)));
        assert_eq!(s.outputs[0].kind, Kind::Hdmi);
        let bt = &s.outputs[3];
        assert_eq!((bt.kind, bt.node), (Kind::Bluetooth, Some(71)));
        assert!((bt.volume - 0.5).abs() < 0.001);
        assert_eq!((s.outputs[4].kind, s.outputs[4].node, s.outputs[4].device), (Kind::Virtual, Some(90), None));
        // The source default comes as a JSON string (older PipeWire).
        assert_eq!(s.inputs.len(), 1);
        assert_eq!((s.inputs[0].name.as_str(), s.inputs[0].default, s.inputs[0].node), ("Microfone interno", true, Some(57)));
    }

    /// After switching to the headphones some cards still report the old profile: the
    /// port that plays is the one in "Route", with its sink.
    #[test]
    fn stale_profile_does_not_hide_the_playing_port() {
        let dump = DUMP
            .replace(r#"{"index": 3, "direction": "Output", "device": 3, "profiles": [2]},"#, r#"{"index": 6, "direction": "Output", "device": 6, "profiles": [1]},"#)
            .replace(r#""node.name": "alsa_output.speaker", "device.id": 48, "card.profile.device": 3"#, r#""node.name": "alsa_output.headphones", "device.id": 48, "card.profile.device": 6"#);
        let s = parse_dump(&dump).unwrap();
        let phones = s.outputs.iter().find(|p| p.kind == Kind::Headphones).unwrap();
        assert_eq!(phones.node, Some(55));
        let speaker = s.outputs.iter().find(|p| p.route == Some(3)).unwrap();
        assert_eq!(speaker.node, None);
    }

    #[test]
    fn picks_the_profile_with_the_port() {
        let v: Value = serde_json::from_str(DUMP).unwrap();
        let dump = v.as_array().unwrap();
        let s = Snapshot { outputs: ports_of(dump, Direction::Output), inputs: Vec::new() };
        let card = &dump[0];
        assert_eq!(best_profile(card, &s.outputs[2]), Some(1));
        // HDMI1 is in both: the one with the highest priority.
        assert_eq!(best_profile(card, &s.outputs[0]), Some(1));
    }

    /// `cargo test -p glove-config -- --ignored --nocapture this_computer`
    #[test]
    #[ignore]
    fn this_computer() {
        let s = snapshot().expect("pw-dump");
        for p in s.outputs.iter().chain(&s.inputs) {
            println!("{:?}", p);
        }
    }

    /// Switches the real output to `GLOVE_AUDIO_SELECT` (a part of the name), then shows
    /// the outputs: `GLOVE_AUDIO_SELECT=Fones cargo test -p glove-config -- --ignored --nocapture select_on_this_computer`
    #[test]
    #[ignore]
    fn select_on_this_computer() {
        let want = std::env::var("GLOVE_AUDIO_SELECT").expect("GLOVE_AUDIO_SELECT");
        let s = snapshot().expect("pw-dump");
        let p = s.outputs.iter().find(|p| p.name.contains(&want)).expect("no such output");
        let t = Instant::now();
        let ok = select(p, Direction::Output);
        println!("select {} -> {ok} in {:?}", p.name, t.elapsed());
        for p in snapshot().unwrap().outputs {
            println!("{} node={:?} default={}", p.name, p.node, p.default);
        }
    }

    #[test]
    fn bad_json_is_none() {
        assert!(parse_dump("not json").is_none());
        assert_eq!(parse_dump("[]").unwrap(), Snapshot::default());
    }
}
