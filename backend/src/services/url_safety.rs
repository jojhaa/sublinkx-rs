use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use url::Url;

use crate::errors::AppError;

pub struct ValidatedPublicHttpUrl {
    url: Url,
    host: String,
    resolved_addrs: Vec<SocketAddr>,
}

impl ValidatedPublicHttpUrl {
    pub fn as_str(&self) -> &str {
        self.url.as_str()
    }

    pub fn pin_reqwest_resolver(&self, builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
        if self.host.parse::<IpAddr>().is_ok() {
            builder
        } else {
            builder.resolve_to_addrs(&self.host, &self.resolved_addrs)
        }
    }
}

pub async fn validate_public_http_url(
    value: &str,
    label: &str,
) -> Result<ValidatedPublicHttpUrl, AppError> {
    let url = parse_http_url(value, label)?;
    let host = url
        .host_str()
        .ok_or_else(|| AppError::BadRequest(format!("{label} missing host")))?
        .to_string();

    if let Ok(ip) = host.parse::<IpAddr>() {
        reject_blocked_ip(ip, label)?;
        let port = url.port_or_known_default().unwrap_or(80);
        return Ok(ValidatedPublicHttpUrl {
            url,
            host,
            resolved_addrs: vec![SocketAddr::new(ip, port)],
        });
    }

    let port = url.port_or_known_default().unwrap_or(80);
    let resolved = tokio::net::lookup_host((host.as_str(), port))
        .await
        .map_err(|error| AppError::BadRequest(format!("failed to resolve {label} host: {error}")))?
        .collect::<Vec<_>>();
    if resolved.is_empty() {
        return Err(AppError::BadRequest(format!(
            "{label} host did not resolve"
        )));
    }
    for address in &resolved {
        reject_blocked_ip(address.ip(), label)?;
    }

    Ok(ValidatedPublicHttpUrl {
        url,
        host,
        resolved_addrs: resolved,
    })
}

pub fn validate_http_url_format(value: &str, label: &str) -> Result<(), AppError> {
    parse_http_url(value, label).map(|_| ())
}

fn parse_http_url(value: &str, label: &str) -> Result<Url, AppError> {
    if value.chars().any(char::is_control) {
        return Err(AppError::BadRequest(format!(
            "{label} must not contain control characters"
        )));
    }

    let url = Url::parse(value).map_err(|_| AppError::BadRequest(format!("{label} is invalid")))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::BadRequest(format!(
            "{label} must use http or https"
        )));
    }
    if url.host_str().is_none() {
        return Err(AppError::BadRequest(format!("{label} missing host")));
    }

    Ok(url)
}

fn reject_blocked_ip(ip: IpAddr, label: &str) -> Result<(), AppError> {
    let blocked = match ip {
        IpAddr::V4(ip) => is_blocked_ipv4(ip),
        IpAddr::V6(ip) => is_blocked_ipv6(ip),
    };
    if blocked {
        return Err(AppError::BadRequest(format!(
            "{label} resolves to a private or local address"
        )));
    }
    Ok(())
}

fn is_blocked_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.octets()[0] == 0
}

fn is_blocked_ipv6(ip: Ipv6Addr) -> bool {
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.to_ipv4_mapped().is_some_and(is_blocked_ipv4)
        || matches!(ip.segments()[0] & 0xfe00, 0xfc00 | 0xfe00)
}

#[cfg(test)]
mod tests {
    use super::{validate_http_url_format, validate_public_http_url};

    #[test]
    fn rejects_control_characters_in_url_format() {
        assert!(validate_http_url_format("https://example.com/\nnext", "test url").is_err());
    }

    #[tokio::test]
    async fn rejects_local_public_http_url() {
        let result = validate_public_http_url("http://127.0.0.1:8080/sub", "test url").await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn rejects_ipv4_mapped_local_public_http_url() {
        let result =
            validate_public_http_url("http://[::ffff:127.0.0.1]:8080/sub", "test url").await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn validated_ip_url_records_single_addr() {
        let validated = validate_public_http_url("http://1.1.1.1/sub", "test url")
            .await
            .unwrap();

        assert_eq!(validated.as_str(), "http://1.1.1.1/sub");
        assert_eq!(validated.resolved_addrs.len(), 1);
    }
}
