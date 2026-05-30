use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    Shadowsocks,
    ShadowsocksR,
    Socks5,
    Http,
    Vmess,
    Vless,
    Trojan,
    Hysteria,
    Hysteria2,
    Tuic,
    WireGuard,
    Snell,
    AnyTls,
    Naive,
    Ssh,
    Juicity,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SupportLevel {
    Supported,
    Conditional,
    NeedsVerification,
    Unsupported,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolCapabilities {
    pub mihomo: SupportLevel,
    pub stash: SupportLevel,
    pub surge: SupportLevel,
    pub xray_family: SupportLevel,
    pub sing_box_family: SupportLevel,
    pub shadowrocket: SupportLevel,
}

impl Protocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Protocol::Shadowsocks => "shadowsocks",
            Protocol::ShadowsocksR => "shadowsocks_r",
            Protocol::Socks5 => "socks5",
            Protocol::Http => "http",
            Protocol::Vmess => "vmess",
            Protocol::Vless => "vless",
            Protocol::Trojan => "trojan",
            Protocol::Hysteria => "hysteria",
            Protocol::Hysteria2 => "hysteria2",
            Protocol::Tuic => "tuic",
            Protocol::WireGuard => "wireguard",
            Protocol::Snell => "snell",
            Protocol::AnyTls => "anytls",
            Protocol::Naive => "naive",
            Protocol::Ssh => "ssh",
            Protocol::Juicity => "juicity",
        }
    }
}
