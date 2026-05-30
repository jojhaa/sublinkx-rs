use axum::{
    http::{HeaderName, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::json;
use serde_yaml::{Mapping, Value};
use std::collections::HashMap;

use crate::{
    domain::{
        client::{detect_client_target_from_user_agent, resolve_client_target},
        node::NodeView,
        subscription::SubscriptionView,
        template::TemplateRecord,
    },
    errors::AppError,
    repository::{
        group_repo::{self, GroupTable},
        template_repo,
    },
    state::AppState,
};

use super::subscription_service;

const CLASH_ROUTING_TEMPLATE_DOC: &str = include_str!("../../../docs/clash-routing-template.md");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportMode {
    Strict,
    BestEffort,
}

pub async fn export_subscription(
    state: &AppState,
    token: &str,
    target: Option<&str>,
    mode: Option<&str>,
    user_agent: Option<&str>,
) -> Result<Response, AppError> {
    let mut subscription = subscription_service::get_subscription_by_token(state, token).await?;
    if !subscription.enabled || subscription_service::subscription_is_expired(&subscription) {
        return Err(AppError::NotFound("subscription not found".to_string()));
    }

    sort_subscription_nodes_for_export(state, &mut subscription).await?;

    let export_target = target
        .or_else(|| detect_export_target(user_agent))
        .or(subscription.default_client.as_deref())
        .unwrap_or("xray")
        .to_string();
    let export_mode = parse_export_mode(mode)?;
    let canonical_target = canonical_export_target(&export_target)?;
    let template_target = template_target(&export_target, canonical_target);
    let template = load_export_template(state, subscription.template_id, template_target).await?;

    match canonical_target {
        "xray" => export_xray_bundle(subscription, export_mode),
        "uri-bundle" => export_uri_bundle(subscription, export_mode, &export_target),
        "sssub" => export_sssub(subscription, export_mode),
        "ssd" => export_ssd(subscription, export_mode),
        "mihomo" => export_mihomo(
            subscription,
            export_mode,
            template.as_ref(),
            template_target,
        ),
        "surge" => export_surge(subscription, export_mode, template.as_ref()),
        "sing-box" => export_sing_box(subscription, export_mode, template.as_ref()),
        "quanx" => export_quantumult_x(subscription, export_mode),
        "quan" => export_legacy_client_profile(subscription, export_mode, "Quantumult"),
        "loon" => export_legacy_client_profile(subscription, export_mode, "Loon"),
        "surfboard" => export_legacy_client_profile(subscription, export_mode, "Surfboard"),
        "mellow" => export_mihomo(
            subscription,
            export_mode,
            template.as_ref(),
            template_target,
        ),
        _ => Err(AppError::Internal),
    }
}

async fn sort_subscription_nodes_for_export(
    state: &AppState,
    subscription: &mut SubscriptionView,
) -> Result<(), AppError> {
    let groups = group_repo::list(&state.db, GroupTable::Node).await?;
    let group_order_by_id = groups
        .into_iter()
        .map(|group| (group.id, group.sort_order))
        .collect::<HashMap<_, _>>();

    subscription.nodes.sort_by(|left, right| {
        let left_group_id = left.group_id.unwrap_or(i64::MAX);
        let right_group_id = right.group_id.unwrap_or(i64::MAX);
        let left_group_order = left
            .group_id
            .and_then(|id| group_order_by_id.get(&id).copied())
            .unwrap_or(i64::MAX);
        let right_group_order = right
            .group_id
            .and_then(|id| group_order_by_id.get(&id).copied())
            .unwrap_or(i64::MAX);

        (
            left_group_order,
            left_group_id,
            left.name.to_lowercase(),
            left.id,
        )
            .cmp(&(
                right_group_order,
                right_group_id,
                right.name.to_lowercase(),
                right.id,
            ))
    });
    subscription.node_ids = subscription.nodes.iter().map(|node| node.id).collect();

    Ok(())
}

fn detect_export_target(user_agent: Option<&str>) -> Option<&'static str> {
    detect_client_target_from_user_agent(user_agent).map(|target| target.key)
}

fn template_target<'a>(requested_target: &'a str, canonical_target: &'a str) -> &'a str {
    resolve_client_target(requested_target)
        .map(|target| target.template_kind)
        .unwrap_or(canonical_target)
}

fn export_xray_bundle(
    subscription: SubscriptionView,
    mode: ExportMode,
) -> Result<Response, AppError> {
    let mut lines = Vec::with_capacity(subscription.nodes.len());
    let mut filtered_count = 0usize;

    for node in &subscription.nodes {
        if is_xray_supported(node) {
            lines.push(node.raw_link.trim().to_string());
        } else if matches!(mode, ExportMode::BestEffort) {
            filtered_count += 1;
        } else {
            return Err(AppError::BadRequest(format!(
                "node '{}' is not supported by xray export",
                node.name
            )));
        }
    }

    if lines.is_empty() {
        return Err(AppError::BadRequest(
            "no compatible nodes available for xray export".to_string(),
        ));
    }

    let payload = lines.join("\n");
    let encoded = general_purpose::STANDARD.encode(payload.as_bytes());

    text_response(
        encoded,
        "text/plain; charset=utf-8",
        &format!("{}.txt", subscription.name),
        mode,
        filtered_count,
    )
}

fn export_uri_bundle(
    subscription: SubscriptionView,
    mode: ExportMode,
    target: &str,
) -> Result<Response, AppError> {
    let mut lines = Vec::with_capacity(subscription.nodes.len());
    let mut filtered_count = 0usize;

    for node in &subscription.nodes {
        if is_uri_bundle_supported(node, target) {
            lines.push(node.raw_link.trim().to_string());
        } else if matches!(mode, ExportMode::BestEffort) {
            filtered_count += 1;
        } else {
            return Err(AppError::BadRequest(format!(
                "node '{}' is not supported by {} export",
                node.name, target
            )));
        }
    }

    if lines.is_empty() {
        return Err(AppError::BadRequest(format!(
            "no compatible nodes available for {} export",
            target
        )));
    }

    text_response(
        lines.join("\n"),
        "text/plain; charset=utf-8",
        &format!("{}.txt", subscription.name),
        mode,
        filtered_count,
    )
}

fn export_sssub(subscription: SubscriptionView, mode: ExportMode) -> Result<Response, AppError> {
    let mut servers = Vec::new();
    let mut filtered_count = 0usize;

    for node in &subscription.nodes {
        if node.protocol == "shadowsocks" {
            servers.push(json!({
                "remarks": node.name,
                "server": node.server,
                "server_port": node.port,
                "method": required_json_string(&node.settings, "method")?,
                "password": required_json_string(&node.settings, "password")?
            }));
        } else if matches!(mode, ExportMode::BestEffort) {
            filtered_count += 1;
        } else {
            return Err(AppError::BadRequest(format!(
                "node '{}' is not supported by sssub export",
                node.name
            )));
        }
    }

    if servers.is_empty() {
        return Err(AppError::BadRequest(
            "no compatible nodes available for sssub export".to_string(),
        ));
    }

    let body = serde_json::to_string_pretty(&json!({
        "version": 1,
        "remarks": subscription.name,
        "servers": servers
    }))
    .map_err(|_| AppError::Internal)?;

    text_response(
        body,
        "application/json; charset=utf-8",
        &format!("{}.json", subscription.name),
        mode,
        filtered_count,
    )
}

fn export_ssd(subscription: SubscriptionView, mode: ExportMode) -> Result<Response, AppError> {
    let mut servers = Vec::new();
    let mut filtered_count = 0usize;

    for node in &subscription.nodes {
        if node.protocol == "shadowsocks" {
            servers.push(json!({
                "remarks": node.name,
                "server": node.server,
                "port": node.port,
                "encryption": required_json_string(&node.settings, "method")?,
                "password": required_json_string(&node.settings, "password")?
            }));
        } else if matches!(mode, ExportMode::BestEffort) {
            filtered_count += 1;
        } else {
            return Err(AppError::BadRequest(format!(
                "node '{}' is not supported by ssd export",
                node.name
            )));
        }
    }

    if servers.is_empty() {
        return Err(AppError::BadRequest(
            "no compatible nodes available for ssd export".to_string(),
        ));
    }

    let body = serde_json::to_string_pretty(&json!({
        "airport": subscription.name,
        "port": 0,
        "encryption": "",
        "password": "",
        "traffic_used": 0,
        "traffic_total": 0,
        "expiry": 0,
        "url": "",
        "servers": servers
    }))
    .map_err(|_| AppError::Internal)?;

    text_response(
        body,
        "application/json; charset=utf-8",
        &format!("{}.json", subscription.name),
        mode,
        filtered_count,
    )
}

fn export_mihomo(
    subscription: SubscriptionView,
    mode: ExportMode,
    template: Option<&TemplateRecord>,
    target_name: &str,
) -> Result<Response, AppError> {
    if let Some(template) = template
        && is_upstream_mihomo_template(&template.content)
    {
        let mut root = parse_yaml_template(Some(template), target_name)?;
        yaml_insert_if_missing(
            &mut root,
            "profile-name",
            Value::String(subscription.name.clone()),
        );
        yaml_set_profile_name(&mut root, &subscription.name);
        root.remove(&Value::String("x-sublinkx-upstream-template".to_string()));
        let yaml = serde_yaml::to_string(&root).map_err(|_| AppError::Internal)?;
        return text_response(
            yaml,
            "application/yaml; charset=utf-8",
            &format!("{}.yaml", subscription.name),
            mode,
            0,
        );
    }

    let mut proxy_items = Vec::with_capacity(subscription.nodes.len());
    let mut proxy_names = Vec::with_capacity(subscription.nodes.len());
    let mut filtered_count = 0usize;

    for node in &subscription.nodes {
        if !is_mihomo_supported(node) {
            if matches!(mode, ExportMode::BestEffort) {
                filtered_count += 1;
                continue;
            }

            return Err(AppError::BadRequest(format!(
                "node '{}' is not supported by mihomo export",
                node.name
            )));
        }

        let rendered = render_mihomo_proxy(node)?;
        proxy_names.push(Value::String(node.name.clone()));
        proxy_items.push(Value::Mapping(rendered));
    }

    if proxy_items.is_empty() {
        return Err(AppError::BadRequest(
            "no compatible nodes available for mihomo export".to_string(),
        ));
    }

    let mut root = parse_yaml_template(template, target_name)?;
    yaml_insert_if_missing(
        &mut root,
        "profile-name",
        Value::String(subscription.name.clone()),
    );
    yaml_set_profile_name(&mut root, &subscription.name);
    yaml_insert_if_missing(&mut root, "mixed-port", Value::Number(7890.into()));
    yaml_insert_if_missing(&mut root, "allow-lan", Value::Bool(true));
    yaml_insert_if_missing(&mut root, "mode", Value::String("rule".to_string()));
    yaml_insert_if_missing(&mut root, "log-level", Value::String("info".to_string()));
    yaml_push_sequence(&mut root, "proxies", proxy_items);
    yaml_push_sequence(
        &mut root,
        "proxy-groups",
        vec![Value::Mapping({
            let mut group = Mapping::new();
            group.insert(
                Value::String("name".to_string()),
                Value::String("AUTO".to_string()),
            );
            group.insert(
                Value::String("type".to_string()),
                Value::String("select".to_string()),
            );
            group.insert(
                Value::String("proxies".to_string()),
                Value::Sequence(proxy_names),
            );
            group
        })],
    );
    yaml_push_sequence(
        &mut root,
        "rules",
        vec![Value::String("MATCH,AUTO".to_string())],
    );

    let yaml = serde_yaml::to_string(&root).map_err(|_| AppError::Internal)?;
    text_response(
        yaml,
        "application/yaml; charset=utf-8",
        &format!("{}.yaml", subscription.name),
        mode,
        filtered_count,
    )
}

fn export_surge(
    subscription: SubscriptionView,
    mode: ExportMode,
    template: Option<&TemplateRecord>,
) -> Result<Response, AppError> {
    let mut proxy_lines = Vec::with_capacity(subscription.nodes.len());
    let mut proxy_names = Vec::with_capacity(subscription.nodes.len());
    let mut wireguard_sections = Vec::new();
    let mut filtered_count = 0usize;

    for node in &subscription.nodes {
        if !is_surge_supported(node) {
            if matches!(mode, ExportMode::BestEffort) {
                filtered_count += 1;
                continue;
            }

            return Err(AppError::BadRequest(format!(
                "node '{}' is not supported by surge export",
                node.name
            )));
        }

        match render_surge_proxy(node) {
            Ok(rendered) => {
                proxy_names.push(rendered.name.clone());
                proxy_lines.push(rendered.proxy_line);
                if let Some(section) = rendered.wireguard_section {
                    wireguard_sections.push(section);
                }
            }
            Err(error) if matches!(mode, ExportMode::BestEffort) => {
                filtered_count += 1;
                let _ = error;
            }
            Err(error) => return Err(error),
        }
    }

    if proxy_lines.is_empty() {
        return Err(AppError::BadRequest(
            "no compatible nodes available for surge export".to_string(),
        ));
    }

    let mut output = if let Some(template) = template {
        let mut ini = parse_surge_template(&template.content);
        ensure_section(
            &mut ini.sections,
            "General",
            vec!["dns-server = system, 1.1.1.1, 8.8.8.8".to_string()],
        );
        append_lines_to_section(&mut ini.sections, "Proxy", proxy_lines);
        append_lines_to_section(
            &mut ini.sections,
            "Proxy Group",
            vec![format!(
                "Proxy = select, {}, DIRECT",
                proxy_names.join(", ")
            )],
        );
        append_lines_to_section(&mut ini.sections, "Rule", vec!["FINAL,Proxy".to_string()]);

        for section in wireguard_sections {
            ini.trailing_raw_sections.push(section);
        }

        render_surge_ini(ini)
    } else {
        let mut value = String::from(
            "[General]\n\
dns-server = system, 1.1.1.1, 8.8.8.8\n\n\
[Proxy]\n",
        );
        value.push_str(&proxy_lines.join("\n"));

        if !wireguard_sections.is_empty() {
            value.push('\n');
            value.push('\n');
            value.push_str(&wireguard_sections.join("\n\n"));
        }

        value.push_str("\n\n[Proxy Group]\n");
        value.push_str("Proxy = select, ");
        value.push_str(&proxy_names.join(", "));
        value.push_str(", DIRECT\n\n[Rule]\nFINAL,Proxy\n");
        value
    };

    if !output.ends_with('\n') {
        output.push('\n');
    }

    text_response(
        output,
        "text/plain; charset=utf-8",
        &format!("{}.conf", subscription.name),
        mode,
        filtered_count,
    )
}

fn export_sing_box(
    subscription: SubscriptionView,
    mode: ExportMode,
    template: Option<&TemplateRecord>,
) -> Result<Response, AppError> {
    let mut outbounds = Vec::with_capacity(subscription.nodes.len() + 2);
    let mut selector_tags = Vec::with_capacity(subscription.nodes.len() + 1);
    let mut filtered_count = 0usize;

    for node in &subscription.nodes {
        if !is_sing_box_supported(node) {
            if matches!(mode, ExportMode::BestEffort) {
                filtered_count += 1;
                continue;
            }

            return Err(AppError::BadRequest(format!(
                "node '{}' is not supported by sing-box export",
                node.name
            )));
        }

        match render_sing_box_outbound(node) {
            Ok(outbound) => {
                if let Some(tag) = outbound.get("tag").and_then(|v| v.as_str()) {
                    selector_tags.push(tag.to_string());
                }
                outbounds.push(outbound);
            }
            Err(error) if matches!(mode, ExportMode::BestEffort) => {
                filtered_count += 1;
                let _ = error;
            }
            Err(error) => return Err(error),
        }
    }

    if outbounds.is_empty() {
        return Err(AppError::BadRequest(
            "no compatible nodes available for sing-box export".to_string(),
        ));
    }

    let mut config = parse_json_template(template, "sing-box")?;
    let root = config.as_object_mut().ok_or_else(|| {
        AppError::BadRequest("sing-box template must be a JSON object".to_string())
    })?;
    root.entry("remarks".to_string())
        .or_insert_with(|| json!(subscription.name));
    root.entry("title".to_string())
        .or_insert_with(|| json!(subscription.name));
    let outbounds_array = root
        .entry("outbounds".to_string())
        .or_insert_with(|| serde_json::Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| {
            AppError::BadRequest("sing-box template field 'outbounds' must be an array".to_string())
        })?;

    let existing_tags = outbounds_array
        .iter()
        .filter_map(|item| item.get("tag").and_then(|value| value.as_str()))
        .map(str::to_string)
        .collect::<Vec<_>>();

    outbounds_array.extend(outbounds);

    if !existing_tags.iter().any(|tag| tag == "direct") {
        outbounds_array.push(json!({
            "type": "direct",
            "tag": "direct"
        }));
    }

    if let Some(existing_selector) = outbounds_array
        .iter_mut()
        .find(|item| item.get("tag").and_then(|value| value.as_str()) == Some("select"))
    {
        let selector_outbounds = existing_selector
            .as_object_mut()
            .and_then(|value| value.get_mut("outbounds"))
            .and_then(|value| value.as_array_mut());
        if let Some(selector_outbounds) = selector_outbounds {
            for tag in &selector_tags {
                if !selector_outbounds
                    .iter()
                    .any(|value| value.as_str() == Some(tag.as_str()))
                {
                    selector_outbounds.push(json!(tag));
                }
            }
            if !selector_outbounds
                .iter()
                .any(|value| value.as_str() == Some("direct"))
            {
                selector_outbounds.push(json!("direct"));
            }
        }
    } else {
        let mut selector_members = selector_tags.clone();
        selector_members.push("direct".to_string());
        outbounds_array.push(json!({
            "type": "selector",
            "tag": "select",
            "outbounds": selector_members,
            "default": selector_tags.first().cloned().unwrap_or_else(|| "direct".to_string()),
            "interrupt_exist_connections": false
        }));
    }

    let route = root
        .entry("route".to_string())
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| {
            AppError::BadRequest("sing-box template field 'route' must be an object".to_string())
        })?;
    route
        .entry("auto_detect_interface".to_string())
        .or_insert_with(|| json!(true));
    route
        .entry("final".to_string())
        .or_insert_with(|| json!("select"));

    let body = serde_json::to_string_pretty(&config).map_err(|_| AppError::Internal)?;

    text_response(
        body,
        "application/json; charset=utf-8",
        &format!("{}.json", subscription.name),
        mode,
        filtered_count,
    )
}

fn export_quantumult_x(
    subscription: SubscriptionView,
    mode: ExportMode,
) -> Result<Response, AppError> {
    let mut lines = Vec::new();
    let mut filtered_count = 0usize;

    for node in &subscription.nodes {
        match render_quantumult_x_proxy(node) {
            Ok(line) => lines.push(line),
            Err(error) if matches!(mode, ExportMode::BestEffort) => {
                tracing::debug!(?error, node_id = node.id, "filtered unsupported quanx node");
                filtered_count += 1;
            }
            Err(error) => return Err(error),
        }
    }

    if lines.is_empty() {
        return Err(AppError::BadRequest(
            "no compatible nodes available for quanx export".to_string(),
        ));
    }

    let body = format!(
        "# Profile: {}\n[server_remote]\n{}\n\n[filter_remote]\n\n[rewrite_remote]\n\n[task_local]\n",
        subscription.name,
        lines.join("\n")
    );

    text_response(
        body,
        "text/plain; charset=utf-8",
        &format!("{}.conf", subscription.name),
        mode,
        filtered_count,
    )
}

fn export_legacy_client_profile(
    subscription: SubscriptionView,
    mode: ExportMode,
    client_name: &str,
) -> Result<Response, AppError> {
    let mut proxy_lines = Vec::new();
    let mut proxy_names = Vec::new();
    let mut wireguard_sections = Vec::new();
    let mut filtered_count = 0usize;

    for node in &subscription.nodes {
        match render_surge_proxy(node) {
            Ok(rendered) => {
                proxy_names.push(rendered.name);
                proxy_lines.push(rendered.proxy_line);
                if let Some(section) = rendered.wireguard_section {
                    wireguard_sections.push(section);
                }
            }
            Err(error) if matches!(mode, ExportMode::BestEffort) => {
                tracing::debug!(
                    ?error,
                    node_id = node.id,
                    "filtered unsupported legacy profile node"
                );
                filtered_count += 1;
            }
            Err(error) => return Err(error),
        }
    }

    if proxy_lines.is_empty() {
        return Err(AppError::BadRequest(format!(
            "no compatible nodes available for {} export",
            client_name
        )));
    }

    let mut body = format!(
        "#!MANAGED-CONFIG {} interval=86400 strict=false\n# Profile: {}\n# Client: {}\n\n[General]\nloglevel = notify\n\n[Proxy]\n{}\n\n[Proxy Group]\nProxy = select, {}, DIRECT\n\n[Rule]\nFINAL,Proxy\n",
        subscription.name,
        subscription.name,
        client_name,
        proxy_lines.join("\n"),
        proxy_names.join(", ")
    );

    if !wireguard_sections.is_empty() {
        body.push('\n');
        body.push_str(&wireguard_sections.join("\n\n"));
        body.push('\n');
    }

    text_response(
        body,
        "text/plain; charset=utf-8",
        &format!("{}.conf", subscription.name),
        mode,
        filtered_count,
    )
}

pub(crate) fn render_mihomo_proxy(node: &NodeView) -> Result<Mapping, AppError> {
    let mut map = Mapping::new();
    map.insert(
        Value::String("name".to_string()),
        Value::String(node.name.clone()),
    );
    map.insert(
        Value::String("server".to_string()),
        Value::String(node.server.clone()),
    );
    map.insert(
        Value::String("port".to_string()),
        Value::Number(node.port.into()),
    );

    match node.protocol.as_str() {
        "shadowsocks" => {
            map.insert(
                Value::String("type".to_string()),
                Value::String("ss".to_string()),
            );
            insert_json_string(&mut map, "cipher", &node.settings, "method")?;
            insert_json_string(&mut map, "password", &node.settings, "password")?;
        }
        "vmess" => {
            map.insert(
                Value::String("type".to_string()),
                Value::String("vmess".to_string()),
            );
            insert_json_string(&mut map, "uuid", &node.settings, "id")?;
            map.insert(
                Value::String("alterId".to_string()),
                Value::Number(
                    node.settings
                        .get("aid")
                        .and_then(json_to_i64)
                        .unwrap_or(0)
                        .into(),
                ),
            );
            map.insert(
                Value::String("cipher".to_string()),
                Value::String(
                    node.settings
                        .get("scy")
                        .and_then(|v| v.as_str())
                        .unwrap_or("auto")
                        .to_string(),
                ),
            );
            if let Some(network) = node.settings.get("net").and_then(|v| v.as_str()) {
                map.insert(
                    Value::String("network".to_string()),
                    Value::String(network.to_string()),
                );
            }
            if let Some(path) = node.settings.get("path").and_then(|v| v.as_str())
                && !path.is_empty()
            {
                map.insert(
                    Value::String("ws-opts".to_string()),
                    Value::Mapping({
                        let mut ws = Mapping::new();
                        ws.insert(
                            Value::String("path".to_string()),
                            Value::String(path.to_string()),
                        );
                        if let Some(host) = node.settings.get("host").and_then(|v| v.as_str())
                            && !host.is_empty()
                        {
                            ws.insert(
                                Value::String("headers".to_string()),
                                Value::Mapping({
                                    let mut headers = Mapping::new();
                                    headers.insert(
                                        Value::String("Host".to_string()),
                                        Value::String(host.to_string()),
                                    );
                                    headers
                                }),
                            );
                        }
                        ws
                    }),
                );
            }
            if let Some(tls) = node.settings.get("tls").and_then(|v| v.as_str())
                && tls.eq_ignore_ascii_case("tls")
            {
                map.insert(Value::String("tls".to_string()), Value::Bool(true));
            }
            if let Some(sni) = node.settings.get("sni").and_then(|v| v.as_str())
                && !sni.is_empty()
            {
                map.insert(
                    Value::String("servername".to_string()),
                    Value::String(sni.to_string()),
                );
            }
        }
        "vless" => {
            map.insert(
                Value::String("type".to_string()),
                Value::String("vless".to_string()),
            );
            insert_json_string(&mut map, "uuid", &node.settings, "uuid")?;
            map.insert(
                Value::String("encryption".to_string()),
                Value::String(mihomo_vless_encryption(&node.settings).to_string()),
            );
            if let Some(flow) = node.settings.get("flow").and_then(|v| v.as_str())
                && !flow.is_empty()
            {
                map.insert(
                    Value::String("flow".to_string()),
                    Value::String(flow.to_string()),
                );
            }
            if let Some(network) = node.settings.get("type").and_then(|v| v.as_str())
                && !network.is_empty()
            {
                map.insert(
                    Value::String("network".to_string()),
                    Value::String(network.to_string()),
                );
            }
            if node
                .settings
                .get("type")
                .and_then(|v| v.as_str())
                .is_some_and(|network| network.eq_ignore_ascii_case("grpc"))
            {
                if let Some(service_name) = node
                    .settings
                    .get("grpc-service-name")
                    .and_then(|v| v.as_str())
                    && !service_name.is_empty()
                {
                    map.insert(
                        Value::String("grpc-opts".to_string()),
                        Value::Mapping({
                            let mut grpc = Mapping::new();
                            grpc.insert(
                                Value::String("grpc-service-name".to_string()),
                                Value::String(service_name.to_string()),
                            );
                            grpc
                        }),
                    );
                }
            }
            if let Some(path) = node.settings.get("path").and_then(|v| v.as_str())
                && !path.is_empty()
            {
                map.insert(
                    Value::String("ws-opts".to_string()),
                    Value::Mapping({
                        let mut ws = Mapping::new();
                        ws.insert(
                            Value::String("path".to_string()),
                            Value::String(path.to_string()),
                        );
                        if let Some(host) = node.settings.get("host").and_then(|v| v.as_str())
                            && !host.is_empty()
                        {
                            ws.insert(
                                Value::String("headers".to_string()),
                                Value::Mapping({
                                    let mut headers = Mapping::new();
                                    headers.insert(
                                        Value::String("Host".to_string()),
                                        Value::String(host.to_string()),
                                    );
                                    headers
                                }),
                            );
                        }
                        ws
                    }),
                );
            }
            if let Some(security) = node.settings.get("security").and_then(|v| v.as_str())
                && matches!(security, "tls" | "reality")
            {
                map.insert(Value::String("tls".to_string()), Value::Bool(true));
            }
            if let Some(sni) = node.settings.get("sni").and_then(|v| v.as_str())
                && !sni.is_empty()
            {
                map.insert(
                    Value::String("servername".to_string()),
                    Value::String(sni.to_string()),
                );
            }
            if let Some(fp) = node.settings.get("fp").and_then(|v| v.as_str())
                && !fp.is_empty()
            {
                map.insert(
                    Value::String("client-fingerprint".to_string()),
                    Value::String(fp.to_string()),
                );
            }
            map.insert(Value::String("udp".to_string()), Value::Bool(true));
            if let Some(insecure) = node.settings.get("insecure").and_then(json_to_bool) {
                map.insert(
                    Value::String("skip-cert-verify".to_string()),
                    Value::Bool(insecure),
                );
            }
            insert_optional_json_string(
                &mut map,
                "packet-encoding",
                &node.settings,
                "packet-encoding",
            );
            if let Some(pbk) = node.settings.get("pbk").and_then(|v| v.as_str())
                && !pbk.is_empty()
            {
                map.insert(
                    Value::String("reality-opts".to_string()),
                    Value::Mapping({
                        let mut reality = Mapping::new();
                        reality.insert(
                            Value::String("public-key".to_string()),
                            Value::String(pbk.to_string()),
                        );
                        if let Some(sid) = node.settings.get("sid").and_then(|v| v.as_str())
                            && !sid.is_empty()
                        {
                            reality.insert(
                                Value::String("short-id".to_string()),
                                Value::String(sid.to_string()),
                            );
                        }
                        reality
                    }),
                );
            }
        }
        "anytls" => {
            map.insert(
                Value::String("type".to_string()),
                Value::String("anytls".to_string()),
            );
            insert_json_string(&mut map, "password", &node.settings, "password")?;
            map.insert(Value::String("udp".to_string()), Value::Bool(true));
            insert_optional_json_string(&mut map, "sni", &node.settings, "sni");
            if let Some(fp) = node.settings.get("fp").and_then(|v| v.as_str())
                && !fp.is_empty()
            {
                map.insert(
                    Value::String("client-fingerprint".to_string()),
                    Value::String(fp.to_string()),
                );
            }
            insert_optional_json_bool(&mut map, "skip-cert-verify", &node.settings, "insecure");
            insert_optional_json_i64(
                &mut map,
                "idle-session-check-interval",
                &node.settings,
                "idle-session-check-interval",
            );
            insert_optional_json_i64(
                &mut map,
                "idle-session-timeout",
                &node.settings,
                "idle-session-timeout",
            );
            insert_optional_json_i64(
                &mut map,
                "min-idle-session",
                &node.settings,
                "min-idle-session",
            );
            if let Some(alpn) = node.settings.get("alpn").and_then(|v| v.as_str())
                && !alpn.is_empty()
            {
                map.insert(
                    Value::String("alpn".to_string()),
                    Value::Sequence(split_csv(alpn)),
                );
            }
        }
        "trojan" => {
            map.insert(
                Value::String("type".to_string()),
                Value::String("trojan".to_string()),
            );
            insert_json_string(&mut map, "password", &node.settings, "password")?;
            if let Some(network) = node.settings.get("type").and_then(|v| v.as_str())
                && !network.is_empty()
            {
                map.insert(
                    Value::String("network".to_string()),
                    Value::String(network.to_string()),
                );
            }
            if let Some(path) = node.settings.get("path").and_then(|v| v.as_str())
                && !path.is_empty()
            {
                map.insert(
                    Value::String("ws-opts".to_string()),
                    Value::Mapping({
                        let mut ws = Mapping::new();
                        ws.insert(
                            Value::String("path".to_string()),
                            Value::String(path.to_string()),
                        );
                        if let Some(host) = node.settings.get("host").and_then(|v| v.as_str())
                            && !host.is_empty()
                        {
                            ws.insert(
                                Value::String("headers".to_string()),
                                Value::Mapping({
                                    let mut headers = Mapping::new();
                                    headers.insert(
                                        Value::String("Host".to_string()),
                                        Value::String(host.to_string()),
                                    );
                                    headers
                                }),
                            );
                        }
                        ws
                    }),
                );
            }
            map.insert(Value::String("tls".to_string()), Value::Bool(true));
            if let Some(sni) = node.settings.get("sni").and_then(|v| v.as_str())
                && !sni.is_empty()
            {
                map.insert(
                    Value::String("servername".to_string()),
                    Value::String(sni.to_string()),
                );
            }
        }
        "hysteria2" => {
            map.insert(
                Value::String("type".to_string()),
                Value::String("hysteria2".to_string()),
            );
            insert_json_string(&mut map, "password", &node.settings, "password")?;
            if let Some(sni) = node.settings.get("sni").and_then(|v| v.as_str())
                && !sni.is_empty()
            {
                map.insert(
                    Value::String("sni".to_string()),
                    Value::String(sni.to_string()),
                );
            }
            if let Some(insecure) = node.settings.get("insecure").and_then(json_to_bool) {
                map.insert(
                    Value::String("skip-cert-verify".to_string()),
                    Value::Bool(insecure),
                );
            }
            if let Some(obfs) = node.settings.get("obfs").and_then(|v| v.as_str())
                && !obfs.is_empty()
            {
                map.insert(
                    Value::String("obfs".to_string()),
                    Value::String(obfs.to_string()),
                );
            }
            if let Some(obfs_password) = node.settings.get("obfs-password").and_then(|v| v.as_str())
                && !obfs_password.is_empty()
            {
                map.insert(
                    Value::String("obfs-password".to_string()),
                    Value::String(obfs_password.to_string()),
                );
            }
            if let Some(ports) = node.settings.get("ports").and_then(|v| v.as_str())
                && !ports.is_empty()
            {
                map.insert(
                    Value::String("ports".to_string()),
                    Value::String(ports.to_string()),
                );
            }
            if let Some(alpn) = node.settings.get("alpn").and_then(|v| v.as_str())
                && !alpn.is_empty()
            {
                map.insert(
                    Value::String("alpn".to_string()),
                    Value::Sequence(split_csv(alpn)),
                );
            }
            if let Some(up) = node.settings.get("up").and_then(json_to_i64) {
                map.insert(Value::String("up".to_string()), Value::Number(up.into()));
            }
            if let Some(down) = node.settings.get("down").and_then(json_to_i64) {
                map.insert(
                    Value::String("down".to_string()),
                    Value::Number(down.into()),
                );
            }
        }
        "tuic" => {
            map.insert(
                Value::String("type".to_string()),
                Value::String("tuic".to_string()),
            );
            insert_json_string(&mut map, "uuid", &node.settings, "uuid")?;
            insert_json_string(&mut map, "password", &node.settings, "password")?;
            if let Some(sni) = node.settings.get("sni").and_then(|v| v.as_str())
                && !sni.is_empty()
            {
                map.insert(
                    Value::String("sni".to_string()),
                    Value::String(sni.to_string()),
                );
            }
            if let Some(insecure) = node.settings.get("insecure").and_then(json_to_bool) {
                map.insert(
                    Value::String("skip-cert-verify".to_string()),
                    Value::Bool(insecure),
                );
            }
            if let Some(alpn) = node.settings.get("alpn").and_then(|v| v.as_str())
                && !alpn.is_empty()
            {
                map.insert(
                    Value::String("alpn".to_string()),
                    Value::Sequence(split_csv(alpn)),
                );
            }
            if let Some(cc) = node
                .settings
                .get("congestion-controller")
                .and_then(|v| v.as_str())
                && !cc.is_empty()
            {
                map.insert(
                    Value::String("congestion-controller".to_string()),
                    Value::String(cc.to_string()),
                );
            }
            if let Some(mode) = node.settings.get("udp-relay-mode").and_then(|v| v.as_str())
                && !mode.is_empty()
            {
                map.insert(
                    Value::String("udp-relay-mode".to_string()),
                    Value::String(mode.to_string()),
                );
            }
        }
        "wireguard" => {
            map.insert(
                Value::String("type".to_string()),
                Value::String("wireguard".to_string()),
            );
            insert_json_string(&mut map, "public-key", &node.settings, "public-key")?;
            if let Some(private_key) = node.settings.get("private-key").and_then(|v| v.as_str())
                && !private_key.is_empty()
            {
                map.insert(
                    Value::String("private-key".to_string()),
                    Value::String(private_key.to_string()),
                );
            }
            if let Some(psk) = node.settings.get("pre-shared-key").and_then(|v| v.as_str())
                && !psk.is_empty()
            {
                map.insert(
                    Value::String("pre-shared-key".to_string()),
                    Value::String(psk.to_string()),
                );
            }

            let mut ip_values = Vec::new();
            if let Some(ip) = node.settings.get("ip").and_then(|v| v.as_str()) {
                ip_values.extend(split_csv(ip));
            }
            if let Some(ipv6) = node.settings.get("ipv6").and_then(|v| v.as_str()) {
                ip_values.extend(split_csv(ipv6));
            }
            if !ip_values.is_empty() {
                map.insert(Value::String("ip".to_string()), Value::Sequence(ip_values));
            }

            if let Some(mtu) = node.settings.get("mtu").and_then(json_to_i64) {
                map.insert(Value::String("mtu".to_string()), Value::Number(mtu.into()));
            }
            if let Some(udp) = node.settings.get("udp").and_then(json_to_bool) {
                map.insert(Value::String("udp".to_string()), Value::Bool(udp));
            }
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "node '{}' uses unsupported mihomo protocol {}",
                node.name, other
            )));
        }
    }

    Ok(map)
}

fn is_xray_supported(node: &NodeView) -> bool {
    matches!(
        node.protocol.as_str(),
        "shadowsocks" | "vmess" | "vless" | "trojan" | "hysteria2"
    )
}

fn is_uri_bundle_supported(node: &NodeView, target: &str) -> bool {
    match target {
        "ss" => node.protocol == "shadowsocks",
        "ssr" => matches!(node.protocol.as_str(), "shadowsocks_r" | "ssr"),
        "trojan" => node.protocol == "trojan",
        "mixed" => !node.raw_link.trim().is_empty(),
        _ => matches!(
            node.protocol.as_str(),
            "shadowsocks"
                | "vmess"
                | "vless"
                | "trojan"
                | "hysteria2"
                | "tuic"
                | "wireguard"
                | "anytls"
        ),
    }
}

fn is_mihomo_supported(node: &NodeView) -> bool {
    matches!(
        node.protocol.as_str(),
        "shadowsocks"
            | "vmess"
            | "vless"
            | "trojan"
            | "hysteria2"
            | "tuic"
            | "wireguard"
            | "anytls"
    )
}

fn is_surge_supported(node: &NodeView) -> bool {
    matches!(
        node.protocol.as_str(),
        "shadowsocks" | "vmess" | "vless" | "trojan" | "hysteria2" | "tuic" | "wireguard"
    )
}

fn is_sing_box_supported(node: &NodeView) -> bool {
    matches!(
        node.protocol.as_str(),
        "shadowsocks"
            | "vmess"
            | "vless"
            | "trojan"
            | "hysteria2"
            | "tuic"
            | "wireguard"
            | "anytls"
    )
}

fn parse_export_mode(mode: Option<&str>) -> Result<ExportMode, AppError> {
    match mode.unwrap_or("strict") {
        "strict" => Ok(ExportMode::Strict),
        "best_effort" => Ok(ExportMode::BestEffort),
        other => Err(AppError::BadRequest(format!(
            "unsupported export mode: {}",
            other
        ))),
    }
}

pub(crate) struct SurgeRenderResult {
    pub(crate) name: String,
    pub(crate) proxy_line: String,
    pub(crate) wireguard_section: Option<String>,
}

pub(crate) fn render_sing_box_outbound(node: &NodeView) -> Result<serde_json::Value, AppError> {
    let tag = sanitize_policy_name(&node.name, node.id);

    let mut outbound = json!({
        "tag": tag,
        "server": node.server,
        "server_port": node.port,
    });

    match node.protocol.as_str() {
        "shadowsocks" => {
            outbound["type"] = json!("shadowsocks");
            outbound["method"] = json!(required_json_string(&node.settings, "method")?);
            outbound["password"] = json!(required_json_string(&node.settings, "password")?);
        }
        "vmess" => {
            outbound["type"] = json!("vmess");
            outbound["uuid"] = json!(required_json_string(&node.settings, "id")?);
            outbound["security"] = json!(
                node.settings
                    .get("scy")
                    .and_then(|v| v.as_str())
                    .unwrap_or("auto")
            );
            outbound["alter_id"] =
                json!(node.settings.get("aid").and_then(json_to_i64).unwrap_or(0));
            apply_v2ray_transport(&mut outbound, &node.settings);
            apply_tls_config(&mut outbound, &node.settings, &node.server);
        }
        "vless" => {
            outbound["type"] = json!("vless");
            outbound["uuid"] = json!(required_json_string(&node.settings, "uuid")?);
            if let Some(flow) = node.settings.get("flow").and_then(|v| v.as_str())
                && !flow.is_empty()
            {
                outbound["flow"] = json!(flow);
            }
            apply_v2ray_transport(&mut outbound, &node.settings);
            apply_tls_config(&mut outbound, &node.settings, &node.server);
            outbound["packet_encoding"] = json!("xudp");
        }
        "anytls" => {
            outbound["type"] = json!("anytls");
            outbound["password"] = json!(required_json_string(&node.settings, "password")?);
            apply_forced_tls_config(&mut outbound, &node.settings, &node.server);
            if let Some(value) = node
                .settings
                .get("idle-session-check-interval")
                .and_then(|v| v.as_str())
                && !value.is_empty()
            {
                outbound["idle_session_check_interval"] = json!(value);
            }
            if let Some(value) = node
                .settings
                .get("idle-session-timeout")
                .and_then(|v| v.as_str())
                && !value.is_empty()
            {
                outbound["idle_session_timeout"] = json!(value);
            }
            if let Some(value) = node.settings.get("min-idle-session").and_then(json_to_i64) {
                outbound["min_idle_session"] = json!(value);
            }
        }
        "trojan" => {
            outbound["type"] = json!("trojan");
            outbound["password"] = json!(required_json_string(&node.settings, "password")?);
            apply_v2ray_transport(&mut outbound, &node.settings);
            apply_tls_config(&mut outbound, &node.settings, &node.server);
        }
        "hysteria2" => {
            outbound["type"] = json!("hysteria2");
            outbound["password"] = json!(required_json_string(&node.settings, "password")?);
            if let Some(ports) = node.settings.get("ports").and_then(|v| v.as_str())
                && !ports.is_empty()
            {
                outbound["server_ports"] = json!(ports);
            }
            if let Some(up) = node.settings.get("up").and_then(json_to_i64) {
                outbound["up_mbps"] = json!(up);
            }
            if let Some(down) = node.settings.get("down").and_then(json_to_i64) {
                outbound["down_mbps"] = json!(down);
            }
            if let Some(obfs) = node.settings.get("obfs").and_then(|v| v.as_str())
                && obfs.eq_ignore_ascii_case("salamander")
            {
                if let Some(password) = node.settings.get("obfs-password").and_then(|v| v.as_str())
                    && !password.is_empty()
                {
                    outbound["obfs"] = json!({
                        "type": "salamander",
                        "password": password
                    });
                }
            }
            apply_tls_config(&mut outbound, &node.settings, &node.server);
        }
        "tuic" => {
            outbound["type"] = json!("tuic");
            outbound["uuid"] = json!(required_json_string(&node.settings, "uuid")?);
            outbound["password"] = json!(required_json_string(&node.settings, "password")?);
            if let Some(cc) = node
                .settings
                .get("congestion-controller")
                .and_then(|v| v.as_str())
                && !cc.is_empty()
            {
                outbound["congestion_control"] = json!(cc);
            }
            if let Some(mode) = node.settings.get("udp-relay-mode").and_then(|v| v.as_str())
                && !mode.is_empty()
            {
                outbound["udp_relay_mode"] = json!(mode);
            }
            apply_tls_config(&mut outbound, &node.settings, &node.server);
        }
        "wireguard" => {
            outbound["type"] = json!("wireguard");
            outbound["local_address"] = json!(wireguard_local_addresses(&node.settings)?);
            outbound["private_key"] = json!(required_json_string(&node.settings, "private-key")?);
            outbound["peer_public_key"] =
                json!(required_json_string(&node.settings, "public-key")?);
            if let Some(psk) = node.settings.get("pre-shared-key").and_then(|v| v.as_str())
                && !psk.is_empty()
            {
                outbound["pre_shared_key"] = json!(psk);
            }
            if let Some(mtu) = node.settings.get("mtu").and_then(json_to_i64) {
                outbound["mtu"] = json!(mtu);
            }
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "node '{}' uses unsupported sing-box protocol {}",
                node.name, other
            )));
        }
    }

    Ok(outbound)
}

pub(crate) fn render_surge_proxy(node: &NodeView) -> Result<SurgeRenderResult, AppError> {
    let name = sanitize_policy_name(&node.name, node.id);

    match node.protocol.as_str() {
        "shadowsocks" => {
            let method = required_json_string(&node.settings, "method")?;
            let password = required_json_string(&node.settings, "password")?;
            let mut line = format!(
                "{name} = ss, {}, {}, encrypt-method={}, password={}",
                node.server, node.port, method, password
            );
            if node
                .settings
                .get("udp")
                .and_then(json_to_bool)
                .unwrap_or(false)
            {
                line.push_str(", udp-relay=true");
            }

            Ok(SurgeRenderResult {
                name,
                proxy_line: line,
                wireguard_section: None,
            })
        }
        "vmess" => {
            let uuid = required_json_string(&node.settings, "id")?;
            let mut line = format!(
                "{name} = vmess, {}, {}, username={uuid}",
                node.server, node.port
            );

            if let Some(cipher) = node.settings.get("scy").and_then(|v| v.as_str())
                && !cipher.is_empty()
            {
                line.push_str(&format!(", encrypt-method={cipher}"));
            }
            if let Some(network) = node.settings.get("net").and_then(|v| v.as_str())
                && network.eq_ignore_ascii_case("ws")
            {
                line.push_str(", ws=true");
                if let Some(path) = node.settings.get("path").and_then(|v| v.as_str())
                    && !path.is_empty()
                {
                    line.push_str(&format!(", ws-path={path}"));
                }
                if let Some(host) = node.settings.get("host").and_then(|v| v.as_str())
                    && !host.is_empty()
                {
                    line.push_str(&format!(", ws-headers=Host:{host}"));
                }
            }
            if let Some(aead) = node.settings.get("aid").and_then(json_to_i64)
                && aead == 0
            {
                line.push_str(", vmess-aead=true");
            }
            append_common_tls_params(&mut line, &node.settings, &node.server);

            Ok(SurgeRenderResult {
                name,
                proxy_line: line,
                wireguard_section: None,
            })
        }
        "vless" => {
            let uuid = required_json_string(&node.settings, "uuid")?;
            let mut line = format!(
                "{name} = vless, {}, {}, username={uuid}",
                node.server, node.port
            );

            if let Some(flow) = node.settings.get("flow").and_then(|v| v.as_str())
                && !flow.is_empty()
            {
                line.push_str(&format!(", flow={flow}"));
            }
            if let Some(network) = node.settings.get("type").and_then(|v| v.as_str())
                && network.eq_ignore_ascii_case("ws")
            {
                line.push_str(", ws=true");
                if let Some(path) = node.settings.get("path").and_then(|v| v.as_str())
                    && !path.is_empty()
                {
                    line.push_str(&format!(", ws-path={path}"));
                }
                if let Some(host) = node.settings.get("host").and_then(|v| v.as_str())
                    && !host.is_empty()
                {
                    line.push_str(&format!(", ws-headers=Host:{host}"));
                }
            }
            if let Some(security) = node.settings.get("security").and_then(|v| v.as_str())
                && matches!(security, "tls" | "reality")
            {
                line.push_str(", tls=true");
            }
            if let Some(fp) = node.settings.get("fp").and_then(|v| v.as_str())
                && !fp.is_empty()
            {
                line.push_str(&format!(", client-fingerprint={fp}"));
            }
            if let Some(pbk) = node.settings.get("pbk").and_then(|v| v.as_str())
                && !pbk.is_empty()
            {
                line.push_str(&format!(", reality-public-key={pbk}"));
            }
            if let Some(sid) = node.settings.get("sid").and_then(|v| v.as_str())
                && !sid.is_empty()
            {
                line.push_str(&format!(", reality-short-id={sid}"));
            }
            append_common_tls_params(&mut line, &node.settings, &node.server);

            Ok(SurgeRenderResult {
                name,
                proxy_line: line,
                wireguard_section: None,
            })
        }
        "trojan" => {
            let password = required_json_string(&node.settings, "password")?;
            let mut line = format!(
                "{name} = trojan, {}, {}, password={password}",
                node.server, node.port
            );
            if let Some(network) = node.settings.get("type").and_then(|v| v.as_str())
                && network.eq_ignore_ascii_case("ws")
            {
                line.push_str(", ws=true");
                if let Some(path) = node.settings.get("path").and_then(|v| v.as_str())
                    && !path.is_empty()
                {
                    line.push_str(&format!(", ws-path={path}"));
                }
                if let Some(host) = node.settings.get("host").and_then(|v| v.as_str())
                    && !host.is_empty()
                {
                    line.push_str(&format!(", ws-headers=Host:{host}"));
                }
            }
            append_common_tls_params(&mut line, &node.settings, &node.server);

            Ok(SurgeRenderResult {
                name,
                proxy_line: line,
                wireguard_section: None,
            })
        }
        "hysteria2" => {
            let password = required_json_string(&node.settings, "password")?;
            let mut line = format!(
                "{name} = hysteria2, {}, {}, password={password}",
                node.server, node.port
            );
            if let Some(down) = node.settings.get("down").and_then(json_to_i64) {
                line.push_str(&format!(", download-bandwidth={down}"));
            }
            if let Some(obfs) = node.settings.get("obfs").and_then(|v| v.as_str())
                && obfs.eq_ignore_ascii_case("salamander")
            {
                if let Some(obfs_password) =
                    node.settings.get("obfs-password").and_then(|v| v.as_str())
                    && !obfs_password.is_empty()
                {
                    line.push_str(&format!(", salamander-password={obfs_password}"));
                }
            }
            append_common_tls_params(&mut line, &node.settings, &node.server);

            Ok(SurgeRenderResult {
                name,
                proxy_line: line,
                wireguard_section: None,
            })
        }
        "tuic" => {
            let token = required_json_string(&node.settings, "password")?;
            let mut line = format!(
                "{name} = tuic, {}, {}, token={token}",
                node.server, node.port
            );
            if let Some(alpn) = node.settings.get("alpn").and_then(|v| v.as_str())
                && !alpn.is_empty()
            {
                line.push_str(&format!(", alpn={alpn}"));
            }
            append_common_tls_params(&mut line, &node.settings, &node.server);

            Ok(SurgeRenderResult {
                name,
                proxy_line: line,
                wireguard_section: None,
            })
        }
        "wireguard" => {
            let section_name = format!("WG_{}", sanitize_section_name(&name));
            let private_key = required_json_string(&node.settings, "private-key")?;
            let public_key = required_json_string(&node.settings, "public-key")?;
            let self_ip = node
                .settings
                .get("ip")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let self_ipv6 = node
                .settings
                .get("ipv6")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if self_ip.is_empty() && self_ipv6.is_empty() {
                return Err(AppError::BadRequest(format!(
                    "wireguard node '{}' requires ip or ipv6 for surge export",
                    node.name
                )));
            }

            let mut section = format!("[WireGuard {section_name}]\nprivate-key = {private_key}\n");

            if !self_ip.is_empty() {
                section.push_str(&format!("self-ip = {self_ip}\n"));
            }
            if !self_ipv6.is_empty() {
                section.push_str(&format!("self-ip-v6 = {self_ipv6}\n"));
            }
            if let Some(mtu) = node.settings.get("mtu").and_then(json_to_i64) {
                section.push_str(&format!("mtu = {mtu}\n"));
            }

            let allowed_ips = if !self_ipv6.is_empty() {
                "\"0.0.0.0/0, ::/0\"".to_string()
            } else {
                "0.0.0.0/0".to_string()
            };
            section.push_str(&format!(
                "peer = (public-key = {public_key}, allowed-ips = {allowed_ips}, endpoint = {}:{})",
                node.server, node.port
            ));

            Ok(SurgeRenderResult {
                name: name.clone(),
                proxy_line: format!("{name} = wireguard, section-name = {section_name}"),
                wireguard_section: Some(section),
            })
        }
        other => Err(AppError::BadRequest(format!(
            "node '{}' uses unsupported surge protocol {}",
            node.name, other
        ))),
    }
}

pub(crate) fn render_quantumult_x_proxy(node: &NodeView) -> Result<String, AppError> {
    let tag = sanitize_policy_name(&node.name, node.id);

    match node.protocol.as_str() {
        "shadowsocks" => Ok(format!(
            "shadowsocks={}, {}, method={}, password={}, tag={}",
            node.server,
            node.port,
            required_json_string(&node.settings, "method")?,
            required_json_string(&node.settings, "password")?,
            tag
        )),
        "vmess" => {
            let mut line = format!(
                "vmess={}, {}, method=none, password={}, tag={}",
                node.server,
                node.port,
                required_json_string(&node.settings, "id")?,
                tag
            );
            append_quanx_v2ray_options(&mut line, &node.settings, &node.server);
            Ok(line)
        }
        "vless" => {
            let mut line = format!(
                "vless={}, {}, method=none, password={}, tag={}",
                node.server,
                node.port,
                required_json_string(&node.settings, "uuid")?,
                tag
            );
            if let Some(flow) = node.settings.get("flow").and_then(|v| v.as_str())
                && !flow.is_empty()
            {
                line.push_str(&format!(", flow={flow}"));
            }
            append_quanx_v2ray_options(&mut line, &node.settings, &node.server);
            Ok(line)
        }
        "trojan" => {
            let mut line = format!(
                "trojan={}, {}, password={}, tag={}",
                node.server,
                node.port,
                required_json_string(&node.settings, "password")?,
                tag
            );
            append_quanx_v2ray_options(&mut line, &node.settings, &node.server);
            Ok(line)
        }
        "hysteria2" => {
            let mut line = format!(
                "hysteria2={}, {}, password={}, tag={}",
                node.server,
                node.port,
                required_json_string(&node.settings, "password")?,
                tag
            );
            if let Some(sni) = node.settings.get("sni").and_then(|v| v.as_str())
                && !sni.is_empty()
            {
                line.push_str(&format!(", server_check_url=https://{sni}/"));
            }
            Ok(line)
        }
        other => Err(AppError::BadRequest(format!(
            "node '{}' uses unsupported quanx protocol {}",
            node.name, other
        ))),
    }
}

fn append_quanx_v2ray_options(line: &mut String, settings: &serde_json::Value, server: &str) {
    let network = settings
        .get("type")
        .and_then(|v| v.as_str())
        .or_else(|| settings.get("net").and_then(|v| v.as_str()));

    if let Some(network) = network
        && network.eq_ignore_ascii_case("ws")
    {
        line.push_str(", obfs=ws");
        if let Some(path) = settings.get("path").and_then(|v| v.as_str())
            && !path.is_empty()
        {
            line.push_str(&format!(", obfs-uri={path}"));
        }
        if let Some(host) = settings.get("host").and_then(|v| v.as_str())
            && !host.is_empty()
        {
            line.push_str(&format!(", obfs-host={host}"));
        }
    }

    if let Some(security) = settings.get("security").and_then(|v| v.as_str())
        && matches!(security, "tls" | "reality")
    {
        line.push_str(", over-tls=true");
    }
    if let Some(sni) = settings.get("sni").and_then(|v| v.as_str())
        && !sni.is_empty()
        && sni != server
    {
        line.push_str(&format!(", tls-host={sni}"));
    }
    if let Some(insecure) = settings.get("insecure").and_then(json_to_bool)
        && insecure
    {
        line.push_str(", tls-verification=false");
    }
    if let Some(pbk) = settings.get("pbk").and_then(|v| v.as_str())
        && !pbk.is_empty()
    {
        line.push_str(&format!(", reality-public-key={pbk}"));
    }
    if let Some(sid) = settings.get("sid").and_then(|v| v.as_str())
        && !sid.is_empty()
    {
        line.push_str(&format!(", reality-short-id={sid}"));
    }
}

fn append_common_tls_params(line: &mut String, settings: &serde_json::Value, server: &str) {
    if let Some(sni) = settings.get("sni").and_then(|v| v.as_str()) {
        if !sni.is_empty() && sni != server {
            line.push_str(&format!(", sni={sni}"));
        }
    }
    if let Some(insecure) = settings.get("insecure").and_then(json_to_bool)
        && insecure
    {
        line.push_str(", skip-cert-verify=true");
    }
}

fn canonical_export_target(target: &str) -> Result<&'static str, AppError> {
    match resolve_client_target(target) {
        Some(client) => client.family.as_export_target().ok_or_else(|| {
            AppError::BadRequest(format!(
                "export target '{}' is recognized but not implemented yet",
                target
            ))
        }),
        None => Err(AppError::BadRequest(format!(
            "unsupported export target: {}",
            target
        ))),
    }
}

fn template_kind_applies(template_kind: &str, target: &str) -> bool {
    template_kind == "common"
        || template_kind == target
        || matches!(
            (template_kind, target),
            ("clash", "clash")
                | ("mihomo", "clash")
                | ("mihomo", "mihomo")
                | ("surge", "surge")
                | ("sing-box", "sing-box")
                | ("xray", "xray")
        )
}

async fn load_export_template(
    state: &AppState,
    template_id: Option<i64>,
    target: &str,
) -> Result<Option<TemplateRecord>, AppError> {
    let Some(template_id) = template_id else {
        return Ok(None);
    };

    let template = template_repo::find_by_id(&state.db, template_id)
        .await?
        .ok_or_else(|| AppError::BadRequest(format!("template not found: {}", template_id)))?;

    let applies = template_kind_applies(template.kind.as_str(), target);

    if applies {
        Ok(Some(template))
    } else {
        Ok(None)
    }
}

fn parse_yaml_template(
    template: Option<&TemplateRecord>,
    target_name: &str,
) -> Result<Mapping, AppError> {
    let (template_name, content) = if let Some(template) = template {
        (template.name.as_str(), template.content.as_str())
    } else if target_name == "clash" {
        (
            "built-in Clash routing template",
            built_in_clash_routing_template()?,
        )
    } else {
        return Ok(Mapping::new());
    };

    let value = serde_yaml::from_str::<Value>(content).map_err(|_| {
        AppError::BadRequest(format!(
            "template '{}' is not valid {} YAML",
            template_name, target_name
        ))
    })?;

    match value {
        Value::Mapping(mapping) => Ok(mapping),
        _ => Err(AppError::BadRequest(format!(
            "template '{}' must be a YAML mapping",
            template_name
        ))),
    }
}

fn built_in_clash_routing_template() -> Result<&'static str, AppError> {
    let section_start = CLASH_ROUTING_TEMPLATE_DOC
        .find("## ACL4SSR-Style Full Routing Template")
        .ok_or(AppError::Internal)?;
    let section = &CLASH_ROUTING_TEMPLATE_DOC[section_start..];
    let yaml_start_marker = "```yaml";
    let yaml_start =
        section.find(yaml_start_marker).ok_or(AppError::Internal)? + yaml_start_marker.len();
    let yaml_section = &section[yaml_start..];
    let yaml_end = yaml_section.find("```").ok_or(AppError::Internal)?;

    Ok(yaml_section[..yaml_end].trim())
}

fn yaml_insert_if_missing(root: &mut Mapping, key: &str, value: Value) {
    root.entry(Value::String(key.to_string())).or_insert(value);
}

fn yaml_set_profile_name(root: &mut Mapping, name: &str) {
    let profile_key = Value::String("profile".to_string());
    let mut profile = root
        .remove(&profile_key)
        .and_then(|value| match value {
            Value::Mapping(mapping) => Some(mapping),
            _ => None,
        })
        .unwrap_or_default();

    profile.insert(
        Value::String("name".to_string()),
        Value::String(name.to_string()),
    );
    root.insert(profile_key, Value::Mapping(profile));
}

fn is_upstream_mihomo_template(content: &str) -> bool {
    serde_yaml::from_str::<Value>(content)
        .ok()
        .and_then(|value| {
            let mapping = value.as_mapping()?;
            let marker = mapping
                .get(Value::String("x-sublinkx-upstream-template".to_string()))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let has_proxies = mapping
                .get(Value::String("proxies".to_string()))
                .and_then(Value::as_sequence)
                .is_some_and(|items| !items.is_empty());
            Some(marker && has_proxies)
        })
        .unwrap_or(false)
}

fn yaml_push_sequence(root: &mut Mapping, key: &str, values: Vec<Value>) {
    let entry = root
        .entry(Value::String(key.to_string()))
        .or_insert_with(|| Value::Sequence(Vec::new()));

    if let Value::Sequence(sequence) = entry {
        sequence.extend(values);
    }
}

fn parse_json_template(
    template: Option<&TemplateRecord>,
    target_name: &str,
) -> Result<serde_json::Value, AppError> {
    let Some(template) = template else {
        return Ok(json!({}));
    };

    serde_json::from_str::<serde_json::Value>(&template.content).map_err(|_| {
        AppError::BadRequest(format!(
            "template '{}' is not valid {} JSON",
            template.name, target_name
        ))
    })
}

struct SurgeIni {
    preamble: Vec<String>,
    sections: Vec<SurgeSection>,
    trailing_raw_sections: Vec<String>,
}

struct SurgeSection {
    name: String,
    lines: Vec<String>,
}

fn parse_surge_template(content: &str) -> SurgeIni {
    let mut preamble = Vec::new();
    let mut sections: Vec<SurgeSection> = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_lines: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if let Some(name) = current_name.take() {
                sections.push(SurgeSection {
                    name,
                    lines: std::mem::take(&mut current_lines),
                });
            }
            current_name = Some(trimmed.trim_matches(&['[', ']'][..]).to_string());
        } else if current_name.is_some() {
            current_lines.push(line.to_string());
        } else {
            preamble.push(line.to_string());
        }
    }

    if let Some(name) = current_name {
        sections.push(SurgeSection {
            name,
            lines: current_lines,
        });
    }

    SurgeIni {
        preamble,
        sections,
        trailing_raw_sections: Vec::new(),
    }
}

fn ensure_section(sections: &mut Vec<SurgeSection>, name: &str, lines: Vec<String>) {
    if sections.iter().any(|section| section.name == name) {
        return;
    }

    sections.push(SurgeSection {
        name: name.to_string(),
        lines,
    });
}

fn append_lines_to_section(sections: &mut Vec<SurgeSection>, name: &str, mut lines: Vec<String>) {
    if let Some(section) = sections.iter_mut().find(|section| section.name == name) {
        section.lines.append(&mut lines);
        return;
    }

    sections.push(SurgeSection {
        name: name.to_string(),
        lines,
    });
}

fn render_surge_ini(ini: SurgeIni) -> String {
    let mut output = String::new();

    if !ini.preamble.is_empty() {
        output.push_str(&ini.preamble.join("\n"));
        output.push('\n');
        output.push('\n');
    }

    for (index, section) in ini.sections.iter().enumerate() {
        if index > 0 {
            output.push('\n');
            output.push('\n');
        }
        output.push('[');
        output.push_str(&section.name);
        output.push_str("]\n");
        output.push_str(&section.lines.join("\n"));
    }

    if !ini.trailing_raw_sections.is_empty() {
        if !output.is_empty() {
            output.push('\n');
            output.push('\n');
        }
        output.push_str(&ini.trailing_raw_sections.join("\n\n"));
    }

    output
}

fn required_json_string(settings: &serde_json::Value, key: &str) -> Result<String, AppError> {
    settings
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| AppError::BadRequest(format!("missing settings field: {}", key)))
}

fn sanitize_policy_name(name: &str, id: i64) -> String {
    let cleaned = name
        .chars()
        .map(|ch| match ch {
            ',' | '=' | '\r' | '\n' => ' ',
            _ => ch,
        })
        .collect::<String>()
        .trim()
        .to_string();

    if cleaned.is_empty() {
        format!("Proxy-{id}")
    } else {
        cleaned
    }
}

fn sanitize_section_name(name: &str) -> String {
    let cleaned = name
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>();

    cleaned.trim_matches('_').to_string()
}

fn apply_v2ray_transport(outbound: &mut serde_json::Value, settings: &serde_json::Value) {
    let network = settings
        .get("type")
        .and_then(|v| v.as_str())
        .or_else(|| settings.get("net").and_then(|v| v.as_str()));

    if let Some(network) = network
        && !network.is_empty()
    {
        outbound["transport"] = json!({ "type": network });

        if network.eq_ignore_ascii_case("ws") {
            if let Some(path) = settings.get("path").and_then(|v| v.as_str())
                && !path.is_empty()
            {
                outbound["transport"]["path"] = json!(path);
            }
            if let Some(host) = settings.get("host").and_then(|v| v.as_str())
                && !host.is_empty()
            {
                outbound["transport"]["headers"] = json!({ "Host": host });
            }
        } else if network.eq_ignore_ascii_case("grpc") {
            if let Some(service_name) = settings.get("grpc-service-name").and_then(|v| v.as_str())
                && !service_name.is_empty()
            {
                outbound["transport"]["service_name"] = json!(service_name);
            }
        }
    }
}

fn apply_tls_config(outbound: &mut serde_json::Value, settings: &serde_json::Value, server: &str) {
    let security = settings.get("security").and_then(|v| v.as_str());
    let tls_enabled = security
        .map(|value| matches!(value, "tls" | "reality"))
        .unwrap_or_else(|| {
            settings
                .get("sni")
                .and_then(|v| v.as_str())
                .is_some_and(|value| !value.is_empty())
        });

    if !tls_enabled {
        return;
    }

    apply_forced_tls_config(outbound, settings, server);
}

fn apply_forced_tls_config(
    outbound: &mut serde_json::Value,
    settings: &serde_json::Value,
    server: &str,
) {
    outbound["tls"] = json!({ "enabled": true });

    if let Some(sni) = settings.get("sni").and_then(|v| v.as_str())
        && !sni.is_empty()
        && sni != server
    {
        outbound["tls"]["server_name"] = json!(sni);
    }
    if let Some(insecure) = settings.get("insecure").and_then(json_to_bool)
        && insecure
    {
        outbound["tls"]["insecure"] = json!(true);
    }
    if let Some(alpn) = settings.get("alpn").and_then(|v| v.as_str())
        && !alpn.is_empty()
    {
        let values = alpn
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| json!(value))
            .collect::<Vec<_>>();
        if !values.is_empty() {
            outbound["tls"]["alpn"] = serde_json::Value::Array(values);
        }
    }
    if let Some(fp) = settings.get("fp").and_then(|v| v.as_str())
        && !fp.is_empty()
    {
        outbound["tls"]["utls"] = json!({ "enabled": true, "fingerprint": fp });
    }
    if let Some(pbk) = settings.get("pbk").and_then(|v| v.as_str())
        && !pbk.is_empty()
    {
        outbound["tls"]["reality"] = json!({
            "enabled": true,
            "public_key": pbk
        });
        if let Some(sid) = settings.get("sid").and_then(|v| v.as_str())
            && !sid.is_empty()
        {
            outbound["tls"]["reality"]["short_id"] = json!(sid);
        }
    }
}

fn wireguard_local_addresses(settings: &serde_json::Value) -> Result<Vec<String>, AppError> {
    let mut values = Vec::new();

    if let Some(ip) = settings.get("ip").and_then(|v| v.as_str()) {
        values.extend(
            ip.split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string()),
        );
    }
    if let Some(ipv6) = settings.get("ipv6").and_then(|v| v.as_str()) {
        values.extend(
            ipv6.split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string()),
        );
    }

    if values.is_empty() {
        return Err(AppError::BadRequest(
            "wireguard export requires ip or ipv6".to_string(),
        ));
    }

    Ok(values)
}

fn insert_json_string(
    map: &mut Mapping,
    output_key: &str,
    settings: &serde_json::Value,
    settings_key: &str,
) -> Result<(), AppError> {
    let value = settings
        .get(settings_key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest(format!("missing settings field: {}", settings_key)))?;
    map.insert(
        Value::String(output_key.to_string()),
        Value::String(value.to_string()),
    );
    Ok(())
}

fn insert_optional_json_string(
    map: &mut Mapping,
    output_key: &str,
    settings: &serde_json::Value,
    settings_key: &str,
) {
    if let Some(value) = settings.get(settings_key).and_then(|v| v.as_str())
        && !value.is_empty()
    {
        map.insert(
            Value::String(output_key.to_string()),
            Value::String(value.to_string()),
        );
    }
}

fn insert_optional_json_i64(
    map: &mut Mapping,
    output_key: &str,
    settings: &serde_json::Value,
    settings_key: &str,
) {
    if let Some(value) = settings.get(settings_key).and_then(json_to_i64) {
        map.insert(
            Value::String(output_key.to_string()),
            Value::Number(value.into()),
        );
    }
}

fn insert_optional_json_bool(
    map: &mut Mapping,
    output_key: &str,
    settings: &serde_json::Value,
    settings_key: &str,
) {
    if let Some(value) = settings.get(settings_key).and_then(json_to_bool) {
        map.insert(Value::String(output_key.to_string()), Value::Bool(value));
    }
}

fn json_to_i64(value: &serde_json::Value) -> Option<i64> {
    if let Some(v) = value.as_i64() {
        return Some(v);
    }
    value.as_str().and_then(|v| v.parse::<i64>().ok())
}

fn json_to_bool(value: &serde_json::Value) -> Option<bool> {
    if let Some(v) = value.as_bool() {
        return Some(v);
    }
    value.as_str().and_then(|v| match v {
        "1" | "true" | "TRUE" | "True" => Some(true),
        "0" | "false" | "FALSE" | "False" => Some(false),
        _ => None,
    })
}

fn split_csv(input: &str) -> Vec<Value> {
    input
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| Value::String(value.to_string()))
        .collect()
}

fn mihomo_vless_encryption(settings: &serde_json::Value) -> &str {
    settings
        .get("encryption")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("none")
}

fn text_response(
    body: String,
    content_type: &str,
    filename: &str,
    mode: ExportMode,
    filtered_count: usize,
) -> Result<Response, AppError> {
    let mut response = (StatusCode::OK, body).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type).map_err(|_| AppError::Internal)?,
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&content_disposition_value(filename))
            .map_err(|_| AppError::Internal)?,
    );
    response.headers_mut().insert(
        HeaderName::from_static("x-sublinkx-export-mode"),
        HeaderValue::from_static(match mode {
            ExportMode::Strict => "strict",
            ExportMode::BestEffort => "best_effort",
        }),
    );
    response.headers_mut().insert(
        HeaderName::from_static("x-sublinkx-filtered-count"),
        HeaderValue::from_str(&filtered_count.to_string()).map_err(|_| AppError::Internal)?,
    );
    Ok(response)
}

fn content_disposition_value(filename: &str) -> String {
    let ascii_fallback = ascii_filename_fallback(filename);
    let encoded = utf8_percent_encode(filename, NON_ALPHANUMERIC).to_string();
    format!("inline; filename=\"{ascii_fallback}\"; filename*=UTF-8''{encoded}")
}

fn ascii_filename_fallback(filename: &str) -> String {
    let extension = filename
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .unwrap_or("txt");
    let safe_extension = extension
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();
    let safe_extension = if safe_extension.is_empty() {
        "txt".to_string()
    } else {
        safe_extension
    };

    format!("subscription.{safe_extension}")
}
