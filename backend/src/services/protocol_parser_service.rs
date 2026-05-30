use base64::{Engine as _, engine::general_purpose};
use percent_encoding::percent_decode_str;
use serde_json::json;
use sha2::{Digest, Sha256};
use url::Url;

use crate::{errors::AppError, protocols::Protocol};

pub struct ParsedNode {
    pub protocol: Protocol,
    pub name: String,
    pub server: String,
    pub port: u16,
    pub settings: serde_json::Value,
    pub fingerprint: String,
}

pub fn parse_raw_link(raw_link: &str, custom_name: Option<&str>) -> Result<ParsedNode, AppError> {
    let trimmed = raw_link.trim();
    let url =
        Url::parse(trimmed).map_err(|_| AppError::BadRequest("invalid node link".to_string()))?;

    match url.scheme() {
        "ss" => parse_ss(trimmed, custom_name),
        "vmess" => parse_vmess(trimmed, custom_name),
        "vless" => parse_vless(trimmed, custom_name),
        "trojan" => parse_trojan(trimmed, custom_name),
        "hy2" | "hysteria2" => parse_hysteria2(trimmed, custom_name),
        "tuic" => parse_tuic(trimmed, custom_name),
        "wireguard" => parse_wireguard(trimmed, custom_name),
        "anytls" | "any-tls" => parse_anytls(trimmed, custom_name),
        _ => Err(AppError::BadRequest(format!(
            "unsupported protocol: {}",
            url.scheme()
        ))),
    }
}

fn parse_ss(raw_link: &str, custom_name: Option<&str>) -> Result<ParsedNode, AppError> {
    let without_scheme = raw_link.trim_start_matches("ss://");
    let (main, fragment) = split_fragment(without_scheme);
    let main = main.split('?').next().unwrap_or(main);

    let decoded = general_purpose::STANDARD
        .decode(pad_base64(main))
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_else(|| main.to_string());

    let (userinfo, host_part) = decoded
        .split_once('@')
        .ok_or_else(|| AppError::BadRequest("invalid shadowsocks link".to_string()))?;
    let decoded_userinfo = general_purpose::STANDARD
        .decode(pad_base64(userinfo))
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_else(|| userinfo.to_string());
    let (method, password) = decoded_userinfo
        .split_once(':')
        .ok_or_else(|| AppError::BadRequest("invalid shadowsocks credentials".to_string()))?;
    let (server, port) = split_host_port(host_part)?;
    let name = node_name(custom_name, fragment, "shadowsocks");

    let fingerprint = fingerprint(
        "shadowsocks",
        &format!("{method}|{password}|{server}|{port}"),
    );

    Ok(ParsedNode {
        protocol: Protocol::Shadowsocks,
        name,
        server: server.to_string(),
        port,
        settings: json!({
            "method": method,
            "password": password
        }),
        fingerprint,
    })
}

fn parse_vmess(raw_link: &str, custom_name: Option<&str>) -> Result<ParsedNode, AppError> {
    let encoded = raw_link.trim_start_matches("vmess://");
    let decoded = general_purpose::STANDARD
        .decode(pad_base64(encoded))
        .map_err(|_| AppError::BadRequest("invalid vmess payload".to_string()))?;
    let payload: serde_json::Value = serde_json::from_slice(&decoded)
        .map_err(|_| AppError::BadRequest("invalid vmess json".to_string()))?;

    let server = payload
        .get("add")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("vmess missing server".to_string()))?;
    let port = payload
        .get("port")
        .and_then(json_to_u16)
        .ok_or_else(|| AppError::BadRequest("vmess missing port".to_string()))?;
    let uuid = payload
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("vmess missing id".to_string()))?;
    let name = node_name(
        custom_name,
        payload.get("ps").and_then(|v| v.as_str()),
        "vmess",
    );
    let fingerprint = fingerprint("vmess", &format!("{uuid}|{server}|{port}"));

    Ok(ParsedNode {
        protocol: Protocol::Vmess,
        name,
        server: server.to_string(),
        port,
        settings: payload,
        fingerprint,
    })
}

fn parse_vless(raw_link: &str, custom_name: Option<&str>) -> Result<ParsedNode, AppError> {
    let url =
        Url::parse(raw_link).map_err(|_| AppError::BadRequest("invalid vless link".to_string()))?;
    let server = url
        .host_str()
        .ok_or_else(|| AppError::BadRequest("vless missing server".to_string()))?;
    let port = url
        .port()
        .ok_or_else(|| AppError::BadRequest("vless missing port".to_string()))?;
    let uuid = decode_name(url.username());
    if uuid.is_empty() {
        return Err(AppError::BadRequest("vless missing uuid".to_string()));
    }

    let name = node_name(custom_name, url.fragment(), "vless");
    let query = query_map(&url);
    let fingerprint = fingerprint("vless", &format!("{uuid}|{server}|{port}"));

    Ok(ParsedNode {
        protocol: Protocol::Vless,
        name,
        server: server.to_string(),
        port,
        settings: json!({
            "uuid": uuid,
            "flow": query.get("flow"),
            "encryption": query.get("encryption"),
            "security": query.get("security"),
            "type": query.get("type"),
            "host": query.get("host"),
            "path": query.get("path"),
            "sni": query.get("sni"),
            "alpn": query.get("alpn"),
            "fp": query.get("fp"),
            "pbk": query.get("pbk"),
            "sid": query.get("sid"),
            "insecure": query.get("insecure").or_else(|| query.get("allowInsecure")),
            "packet-encoding": query.get("packet-encoding").or_else(|| query.get("packetEncoding")),
            "grpc-service-name": query.get("serviceName").or_else(|| query.get("service-name")),
            "headerType": query.get("headerType")
        }),
        fingerprint,
    })
}

fn parse_trojan(raw_link: &str, custom_name: Option<&str>) -> Result<ParsedNode, AppError> {
    let url = Url::parse(raw_link)
        .map_err(|_| AppError::BadRequest("invalid trojan link".to_string()))?;
    let server = url
        .host_str()
        .ok_or_else(|| AppError::BadRequest("trojan missing server".to_string()))?;
    let port = url
        .port()
        .ok_or_else(|| AppError::BadRequest("trojan missing port".to_string()))?;
    let password = url.username();
    if password.is_empty() {
        return Err(AppError::BadRequest("trojan missing password".to_string()));
    }

    let name = node_name(custom_name, url.fragment(), "trojan");
    let query = query_map(&url);
    let fingerprint = fingerprint("trojan", &format!("{password}|{server}|{port}"));

    Ok(ParsedNode {
        protocol: Protocol::Trojan,
        name,
        server: server.to_string(),
        port,
        settings: json!({
            "password": password,
            "security": query.get("security"),
            "type": query.get("type"),
            "host": query.get("host"),
            "path": query.get("path"),
            "sni": query.get("sni"),
            "alpn": query.get("alpn"),
            "fp": query.get("fp")
        }),
        fingerprint,
    })
}

fn parse_hysteria2(raw_link: &str, custom_name: Option<&str>) -> Result<ParsedNode, AppError> {
    let url = Url::parse(raw_link)
        .map_err(|_| AppError::BadRequest("invalid hysteria2 link".to_string()))?;
    let server = url
        .host_str()
        .ok_or_else(|| AppError::BadRequest("hysteria2 missing server".to_string()))?;
    let port = url
        .port()
        .ok_or_else(|| AppError::BadRequest("hysteria2 missing port".to_string()))?;
    let query = query_map(&url);
    let password = if !url.username().is_empty() {
        url.username().to_string()
    } else if let Some(value) = query.get("password") {
        value.clone()
    } else {
        return Err(AppError::BadRequest(
            "hysteria2 missing password".to_string(),
        ));
    };

    let name = node_name(custom_name, url.fragment(), "hysteria2");
    let fingerprint = fingerprint("hysteria2", &format!("{password}|{server}|{port}"));

    Ok(ParsedNode {
        protocol: Protocol::Hysteria2,
        name,
        server: server.to_string(),
        port,
        settings: json!({
            "password": password,
            "sni": query.get("sni"),
            "insecure": query.get("insecure")
                .or_else(|| query.get("skip-cert-verify"))
                .or_else(|| query.get("allowInsecure")),
            "obfs": query.get("obfs"),
            "obfs-password": query.get("obfs-password").or_else(|| query.get("obfs_password")),
            "alpn": query.get("alpn"),
            "up": query.get("up"),
            "down": query.get("down"),
            "ports": query.get("ports")
        }),
        fingerprint,
    })
}

fn parse_tuic(raw_link: &str, custom_name: Option<&str>) -> Result<ParsedNode, AppError> {
    let url =
        Url::parse(raw_link).map_err(|_| AppError::BadRequest("invalid tuic link".to_string()))?;
    let server = url
        .host_str()
        .ok_or_else(|| AppError::BadRequest("tuic missing server".to_string()))?;
    let port = url
        .port()
        .ok_or_else(|| AppError::BadRequest("tuic missing port".to_string()))?;
    let uuid = url.username();
    if uuid.is_empty() {
        return Err(AppError::BadRequest("tuic missing uuid".to_string()));
    }
    let query = query_map(&url);
    let password = url
        .password()
        .map(str::to_string)
        .or_else(|| query.get("password").cloned())
        .ok_or_else(|| AppError::BadRequest("tuic missing password".to_string()))?;

    let name = node_name(custom_name, url.fragment(), "tuic");
    let fingerprint = fingerprint("tuic", &format!("{uuid}|{password}|{server}|{port}"));

    Ok(ParsedNode {
        protocol: Protocol::Tuic,
        name,
        server: server.to_string(),
        port,
        settings: json!({
            "uuid": uuid,
            "password": password,
            "sni": query.get("sni"),
            "insecure": query.get("insecure"),
            "alpn": query.get("alpn"),
            "congestion-controller": query.get("congestion_control").or_else(|| query.get("congestion-controller")),
            "udp-relay-mode": query.get("udp_relay_mode").or_else(|| query.get("udp-relay-mode"))
        }),
        fingerprint,
    })
}

fn parse_wireguard(raw_link: &str, custom_name: Option<&str>) -> Result<ParsedNode, AppError> {
    let url = Url::parse(raw_link)
        .map_err(|_| AppError::BadRequest("invalid wireguard link".to_string()))?;
    let server = url
        .host_str()
        .ok_or_else(|| AppError::BadRequest("wireguard missing server".to_string()))?;
    let port = url
        .port()
        .ok_or_else(|| AppError::BadRequest("wireguard missing port".to_string()))?;
    let public_key = url.username();
    if public_key.is_empty() {
        return Err(AppError::BadRequest(
            "wireguard missing public key".to_string(),
        ));
    }

    let query = query_map(&url);
    let name = node_name(custom_name, url.fragment(), "wireguard");
    let fingerprint = fingerprint("wireguard", &format!("{public_key}|{server}|{port}"));

    Ok(ParsedNode {
        protocol: Protocol::WireGuard,
        name,
        server: server.to_string(),
        port,
        settings: json!({
            "public-key": public_key,
            "private-key": query.get("private_key").or_else(|| query.get("private-key")),
            "pre-shared-key": query.get("pre_shared_key").or_else(|| query.get("pre-shared-key")),
            "ip": query.get("ip"),
            "ipv6": query.get("ipv6"),
            "mtu": query.get("mtu"),
            "udp": query.get("udp")
        }),
        fingerprint,
    })
}

fn parse_anytls(raw_link: &str, custom_name: Option<&str>) -> Result<ParsedNode, AppError> {
    let url = Url::parse(raw_link)
        .map_err(|_| AppError::BadRequest("invalid anytls link".to_string()))?;
    let server = url
        .host_str()
        .ok_or_else(|| AppError::BadRequest("anytls missing server".to_string()))?;
    let port = url
        .port()
        .ok_or_else(|| AppError::BadRequest("anytls missing port".to_string()))?;
    let query = query_map(&url);
    let password = if !url.username().is_empty() {
        decode_name(url.username())
    } else if let Some(value) = query.get("password") {
        value.clone()
    } else {
        return Err(AppError::BadRequest("anytls missing password".to_string()));
    };

    let name = node_name(custom_name, url.fragment(), "anytls");
    let fingerprint = fingerprint("anytls", &format!("{password}|{server}|{port}"));

    Ok(ParsedNode {
        protocol: Protocol::AnyTls,
        name,
        server: server.to_string(),
        port,
        settings: json!({
            "password": password,
            "sni": query.get("sni"),
            "alpn": query.get("alpn"),
            "fp": query.get("fp").or_else(|| query.get("fingerprint")),
            "insecure": query.get("insecure").or_else(|| query.get("skip-cert-verify")),
            "udp": query.get("udp"),
            "idle-session-check-interval": query.get("idle_session_check_interval").or_else(|| query.get("idle-session-check-interval")),
            "idle-session-timeout": query.get("idle_session_timeout").or_else(|| query.get("idle-session-timeout")),
            "min-idle-session": query.get("min_idle_session").or_else(|| query.get("min-idle-session"))
        }),
        fingerprint,
    })
}

fn split_host_port(input: &str) -> Result<(&str, u16), AppError> {
    let (server, port_str) = input
        .rsplit_once(':')
        .ok_or_else(|| AppError::BadRequest("missing server port".to_string()))?;
    let port = port_str
        .parse::<u16>()
        .map_err(|_| AppError::BadRequest("invalid server port".to_string()))?;
    Ok((server, port))
}

fn split_fragment(input: &str) -> (&str, Option<&str>) {
    if let Some((main, fragment)) = input.split_once('#') {
        (main, Some(fragment))
    } else {
        (input, None)
    }
}

fn node_name(custom_name: Option<&str>, parsed_name: Option<&str>, fallback: &str) -> String {
    custom_name
        .filter(|value| !value.trim().is_empty())
        .or(parsed_name)
        .map(decode_name)
        .unwrap_or_else(|| fallback.to_string())
}

fn decode_name(input: &str) -> String {
    percent_decode_str(input)
        .decode_utf8()
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|_| input.trim().to_string())
}

fn pad_base64(input: &str) -> String {
    let mut padded = input.replace('-', "+").replace('_', "/");
    let rem = padded.len() % 4;
    if rem != 0 {
        padded.push_str(&"=".repeat(4 - rem));
    }
    padded
}

fn json_to_u16(value: &serde_json::Value) -> Option<u16> {
    if let Some(v) = value.as_u64() {
        return u16::try_from(v).ok();
    }
    value.as_str().and_then(|v| v.parse::<u16>().ok())
}

fn query_map(url: &Url) -> std::collections::BTreeMap<String, String> {
    url.query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

fn fingerprint(protocol: &str, material: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(protocol.as_bytes());
    hasher.update(b":");
    hasher.update(material.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::parse_raw_link;

    #[test]
    fn parses_percent_encoded_vless_uuid() {
        let parsed = parse_raw_link(
            "vless://4c374a1d%2De334%2D4ec1%2Db010%2D489bfa360ba9@example.com:443?type=tcp&security=reality&pbk=abc&sid=123&fp=ios#node",
            None,
        )
        .expect("vless link should parse");

        assert_eq!(
            parsed.settings.get("uuid").and_then(|value| value.as_str()),
            Some("4c374a1d-e334-4ec1-b010-489bfa360ba9")
        );
    }

    #[test]
    fn preserves_hysteria2_extended_fields() {
        let parsed = parse_raw_link(
            "hysteria2://pass@example.com:443?sni=example.com&skip-cert-verify=true&obfs=salamander&obfs-password=secret&ports=20000-30000&up=300&down=300#hy2",
            None,
        )
        .expect("hysteria2 link should parse");

        assert_eq!(
            parsed
                .settings
                .get("insecure")
                .and_then(|value| value.as_str()),
            Some("true")
        );
        assert_eq!(
            parsed
                .settings
                .get("ports")
                .and_then(|value| value.as_str()),
            Some("20000-30000")
        );
        assert_eq!(
            parsed.settings.get("up").and_then(|value| value.as_str()),
            Some("300")
        );
        assert_eq!(
            parsed.settings.get("down").and_then(|value| value.as_str()),
            Some("300")
        );
    }
}
