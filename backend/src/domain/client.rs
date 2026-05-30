use serde::Serialize;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererFamily {
    Xray,
    Mihomo,
    Surge,
    SingBox,
    QuantumultX,
    Quantumult,
    Loon,
    Surfboard,
    Mellow,
    UriBundle,
    SsSub,
    Ssd,
    NotImplemented,
}

impl RendererFamily {
    pub fn as_export_target(self) -> Option<&'static str> {
        match self {
            Self::Xray => Some("xray"),
            Self::Mihomo => Some("mihomo"),
            Self::Surge => Some("surge"),
            Self::SingBox => Some("sing-box"),
            Self::QuantumultX => Some("quanx"),
            Self::Quantumult => Some("quan"),
            Self::Loon => Some("loon"),
            Self::Surfboard => Some("surfboard"),
            Self::Mellow => Some("mellow"),
            Self::UriBundle => Some("uri-bundle"),
            Self::SsSub => Some("sssub"),
            Self::Ssd => Some("ssd"),
            Self::NotImplemented => None,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientTargetStatus {
    Implemented,
    Planned,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ClientTarget {
    pub key: &'static str,
    pub label: &'static str,
    pub family: RendererFamily,
    pub template_kind: &'static str,
    pub status: ClientTargetStatus,
    pub aliases: &'static [&'static str],
    pub user_agent_markers: &'static [&'static str],
}

pub const CLIENT_TARGETS: &[ClientTarget] = &[
    ClientTarget {
        key: "xray",
        label: "Xray URI Bundle",
        family: RendererFamily::Xray,
        template_kind: "xray",
        status: ClientTargetStatus::Implemented,
        aliases: &[
            "xray-uri-bundle",
            "xray_uri_bundle",
            "v2ray",
            "v2rayn",
            "v2rayng",
        ],
        user_agent_markers: &["xray", "v2ray", "v2rayn", "v2rayng"],
    },
    ClientTarget {
        key: "clash",
        label: "Clash",
        family: RendererFamily::Mihomo,
        template_kind: "clash",
        status: ClientTargetStatus::Implemented,
        aliases: &["clash-yaml"],
        user_agent_markers: &["clash"],
    },
    ClientTarget {
        key: "mihomo",
        label: "Mihomo",
        family: RendererFamily::Mihomo,
        template_kind: "mihomo",
        status: ClientTargetStatus::Implemented,
        aliases: &["clash-meta", "clashmeta", "clash-verge", "clash-verge-rev"],
        user_agent_markers: &["mihomo", "clash.meta", "clashmeta", "clash-verge"],
    },
    ClientTarget {
        key: "surge",
        label: "Surge 4/5",
        family: RendererFamily::Surge,
        template_kind: "surge",
        status: ClientTargetStatus::Implemented,
        aliases: &["surge4", "surge5", "surge&ver=4"],
        user_agent_markers: &["surge"],
    },
    ClientTarget {
        key: "sing-box",
        label: "sing-box",
        family: RendererFamily::SingBox,
        template_kind: "sing-box",
        status: ClientTargetStatus::Implemented,
        aliases: &["sing_box", "singbox", "nekobox", "hiddify"],
        user_agent_markers: &["sing-box", "singbox", "nekobox", "hiddify"],
    },
    ClientTarget {
        key: "shadowrocket",
        label: "Shadowrocket",
        family: RendererFamily::Xray,
        template_kind: "xray",
        status: ClientTargetStatus::Implemented,
        aliases: &["shadow-rocket"],
        user_agent_markers: &["shadowrocket"],
    },
    ClientTarget {
        key: "surge3",
        label: "Surge 3",
        family: RendererFamily::Surge,
        template_kind: "surge3",
        status: ClientTargetStatus::Implemented,
        aliases: &["surge&ver=3"],
        user_agent_markers: &[],
    },
    ClientTarget {
        key: "surge2",
        label: "Surge 2",
        family: RendererFamily::Surge,
        template_kind: "surge2",
        status: ClientTargetStatus::Implemented,
        aliases: &["surge&ver=2"],
        user_agent_markers: &[],
    },
    ClientTarget {
        key: "quanx",
        label: "Quantumult X",
        family: RendererFamily::QuantumultX,
        template_kind: "quanx",
        status: ClientTargetStatus::Implemented,
        aliases: &["quantumult-x", "quantumultx"],
        user_agent_markers: &["quantumult x", "quantumultx"],
    },
    ClientTarget {
        key: "quan",
        label: "Quantumult",
        family: RendererFamily::Quantumult,
        template_kind: "quan",
        status: ClientTargetStatus::Implemented,
        aliases: &["quantumult"],
        user_agent_markers: &["quantumult"],
    },
    ClientTarget {
        key: "loon",
        label: "Loon",
        family: RendererFamily::Loon,
        template_kind: "loon",
        status: ClientTargetStatus::Implemented,
        aliases: &[],
        user_agent_markers: &["loon"],
    },
    ClientTarget {
        key: "surfboard",
        label: "Surfboard",
        family: RendererFamily::Surfboard,
        template_kind: "surfboard",
        status: ClientTargetStatus::Implemented,
        aliases: &[],
        user_agent_markers: &["surfboard"],
    },
    ClientTarget {
        key: "mellow",
        label: "Mellow",
        family: RendererFamily::Mellow,
        template_kind: "mellow",
        status: ClientTargetStatus::Implemented,
        aliases: &[],
        user_agent_markers: &["mellow"],
    },
    ClientTarget {
        key: "clashr",
        label: "ClashR",
        family: RendererFamily::Mihomo,
        template_kind: "clashr",
        status: ClientTargetStatus::Implemented,
        aliases: &["clash-r"],
        user_agent_markers: &["clashr", "clash-r"],
    },
    ClientTarget {
        key: "ss",
        label: "Shadowsocks SIP002",
        family: RendererFamily::UriBundle,
        template_kind: "ss",
        status: ClientTargetStatus::Implemented,
        aliases: &["sip002"],
        user_agent_markers: &[],
    },
    ClientTarget {
        key: "sssub",
        label: "Shadowsocks Android SIP008",
        family: RendererFamily::SsSub,
        template_kind: "sssub",
        status: ClientTargetStatus::Implemented,
        aliases: &["sip008", "ss-android"],
        user_agent_markers: &[],
    },
    ClientTarget {
        key: "ssr",
        label: "ShadowsocksR",
        family: RendererFamily::UriBundle,
        template_kind: "ssr",
        status: ClientTargetStatus::Implemented,
        aliases: &["shadowsocksr"],
        user_agent_markers: &[],
    },
    ClientTarget {
        key: "ssd",
        label: "ShadowsocksD",
        family: RendererFamily::Ssd,
        template_kind: "ssd",
        status: ClientTargetStatus::Implemented,
        aliases: &["shadowsocksd"],
        user_agent_markers: &[],
    },
    ClientTarget {
        key: "trojan",
        label: "Trojan URI Bundle",
        family: RendererFamily::UriBundle,
        template_kind: "trojan",
        status: ClientTargetStatus::Implemented,
        aliases: &[],
        user_agent_markers: &[],
    },
    ClientTarget {
        key: "mixed",
        label: "Mixed Subscription",
        family: RendererFamily::UriBundle,
        template_kind: "mixed",
        status: ClientTargetStatus::Implemented,
        aliases: &[],
        user_agent_markers: &[],
    },
];

pub fn resolve_client_target(input: &str) -> Option<&'static ClientTarget> {
    let normalized = normalize_target(input);
    CLIENT_TARGETS.iter().find(|target| {
        normalized == target.key || target.aliases.iter().any(|alias| normalized == *alias)
    })
}

pub fn detect_client_target_from_user_agent(
    user_agent: Option<&str>,
) -> Option<&'static ClientTarget> {
    let agent = user_agent?.to_ascii_lowercase();

    CLIENT_TARGETS
        .iter()
        .filter(|target| target.status == ClientTargetStatus::Implemented)
        .find(|target| {
            target
                .user_agent_markers
                .iter()
                .any(|marker| agent.contains(marker))
        })
}

pub fn is_known_template_kind(kind: &str) -> bool {
    kind == "common"
        || CLIENT_TARGETS
            .iter()
            .any(|target| target.template_kind == kind)
}

fn normalize_target(input: &str) -> String {
    input.trim().to_ascii_lowercase().replace('_', "-")
}
