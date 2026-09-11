use axum::{
    body::{Body, to_bytes},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::json;
use serde_yaml::{Mapping, Value};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    net::IpAddr,
    sync::OnceLock,
    time::{Duration, Instant},
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    db::DbPool,
    domain::{
        client::{detect_client_target_from_user_agent, resolve_client_target},
        node::NodeView,
        node_ip_probe::NodeIpProbeRecord,
        settings::AppSettingsView,
        subscription::SubscriptionView,
        template::TemplateRecord,
    },
    errors::AppError,
    repository::{
        group_repo::{self, GroupTable},
        node_ip_probe_repo, subscription_repo, template_repo,
    },
    state::{AppState, PublicExportCacheEntry},
};

use super::{auth_service, settings_service, subscription_service};

const PUBLIC_EXPORT_CACHE_BODY_LIMIT: usize = 8 * 1024 * 1024;
const PUBLIC_EXPORT_RATE_WINDOW: Duration = Duration::from_secs(60);
const MIHOMO_DEFAULT_DELAY_TEST_URL: &str = "https://cp.cloudflare.com/generate_204";
const MIHOMO_GROUP_PROXY: &str = "代理选择";
const MIHOMO_GROUP_MANUAL: &str = "手动选择";
const MIHOMO_GROUP_AUTO: &str = "自动选择";
const MIHOMO_GROUP_FALLBACK: &str = "故障转移";
const MIHOMO_GROUP_LOAD_BALANCE: &str = "负载均衡";
const MIHOMO_GROUP_ZERO_FRAUD: &str = "0欺诈";
const MIHOMO_GROUP_LOW_FRAUD: &str = "低欺诈";
const MIHOMO_GROUP_RESIDENTIAL_MANUAL: &str = "手动选择住宅网络";
const QUANX_GROUP_PROXY: &str = "节点选择";
const BUILTIN_QUANX_TEMPLATE_NAME: &str = "Built-in Quantumult X Base";
const BUILTIN_CLASH_TEMPLATE_NAME: &str = "Built-in Clash ACL4SSR Style";
const BUILTIN_MIHOMO_TEMPLATE_NAME: &str = "Built-in Mihomo Policy Orchestrator";
pub(crate) const QUANX_DEFAULT_TEMPLATE: &str = "[general]\n\
server_check_url=http://cp.cloudflare.com/generate_204\n\n\
[server_local]\n\n\
[policy]\n\
static=节点选择, 自动选择, direct\n\
url-latency-benchmark=自动选择, server-tag-regex=.*, check-interval=600, tolerance=0, alive-checking=false\n\n\
[filter_remote]\n\n\
[filter_local]\n\
geoip, cn, direct\n\
final, 节点选择\n\n\
[rewrite_remote]\n\n\
[task_local]\n";
const MIHOMO_INLINE_RULE_PROVIDERS_YAML: &str =
    include_str!("../../assets/mihomo-rules-inline.yaml");
static MIHOMO_INLINE_RULE_PROVIDERS_BLOCK: OnceLock<String> = OnceLock::new();
const BUILTIN_CLASH_COMPACT_GROUP_ALIASES: &[(&str, &str)] = &[
    ("手动切换", MIHOMO_GROUP_MANUAL),
    ("Ai平台", "节点选择"),
    ("油管视频", "节点选择"),
    ("奈飞视频", "节点选择"),
    ("电报消息", "节点选择"),
    ("微软服务", "节点选择"),
    ("苹果服务", "节点选择"),
    ("全球直连", "DIRECT"),
    ("广告拦截", "REJECT"),
    ("漏网之鱼", "节点选择"),
];
const BUILTIN_CLASH_REMOVED_GROUPS: &[&str] = &[
    "Ai平台",
    "油管视频",
    "奈飞视频",
    "电报消息",
    "微软服务",
    "苹果服务",
    "全球直连",
    "广告拦截",
    "漏网之鱼",
];
const BUILTIN_MIHOMO_COMPACT_GROUP_ALIASES: &[(&str, &str)] = &[
    ("PROXY", MIHOMO_GROUP_PROXY),
    ("MANUAL", MIHOMO_GROUP_MANUAL),
    ("AUTO", MIHOMO_GROUP_AUTO),
    ("FALLBACK", MIHOMO_GROUP_FALLBACK),
    ("LOAD-BALANCE", MIHOMO_GROUP_LOAD_BALANCE),
    ("AI", MIHOMO_GROUP_PROXY),
    ("STREAMING", MIHOMO_GROUP_PROXY),
    ("GOOGLE", MIHOMO_GROUP_PROXY),
    ("TELEGRAM", MIHOMO_GROUP_PROXY),
    ("MICROSOFT", MIHOMO_GROUP_PROXY),
    ("APPLE", MIHOMO_GROUP_PROXY),
    ("DOMESTIC", "DIRECT"),
    ("GAME", MIHOMO_GROUP_PROXY),
    ("DOWNLOAD-BLOCK", "REJECT"),
    ("FINAL", MIHOMO_GROUP_PROXY),
    ("AI 服务", MIHOMO_GROUP_PROXY),
    ("流媒体", MIHOMO_GROUP_PROXY),
    ("谷歌服务", MIHOMO_GROUP_PROXY),
    ("电报消息", MIHOMO_GROUP_PROXY),
    ("微软服务", MIHOMO_GROUP_PROXY),
    ("苹果服务", MIHOMO_GROUP_PROXY),
    ("国内网站", "DIRECT"),
    ("游戏平台", MIHOMO_GROUP_PROXY),
    ("下载拦截", "REJECT"),
    ("漏网之鱼", MIHOMO_GROUP_PROXY),
];
const BUILTIN_MIHOMO_REMOVED_GROUPS: &[&str] = &[
    "AI",
    "STREAMING",
    "GOOGLE",
    "TELEGRAM",
    "MICROSOFT",
    "APPLE",
    "DOMESTIC",
    "GAME",
    "DOWNLOAD-BLOCK",
    "FINAL",
    "AI 服务",
    "流媒体",
    "谷歌服务",
    "电报消息",
    "微软服务",
    "苹果服务",
    "国内网站",
    "游戏平台",
    "下载拦截",
    "漏网之鱼",
];
const MIHOMO_LEGACY_GROUP_NAMES: &[(&str, &str)] = &[
    ("PROXY", MIHOMO_GROUP_PROXY),
    ("MANUAL", MIHOMO_GROUP_MANUAL),
    ("AUTO", MIHOMO_GROUP_AUTO),
    ("FALLBACK", MIHOMO_GROUP_FALLBACK),
    ("LOAD-BALANCE", MIHOMO_GROUP_LOAD_BALANCE),
    ("AI", "AI 服务"),
    ("STREAMING", "流媒体"),
    ("GOOGLE", "谷歌服务"),
    ("TELEGRAM", "电报消息"),
    ("MICROSOFT", "微软服务"),
    ("APPLE", "苹果服务"),
    ("DOMESTIC", "国内网站"),
    ("GAME", "游戏平台"),
    ("DOWNLOAD-BLOCK", "下载拦截"),
    ("FINAL", "漏网之鱼"),
];
const MIHOMO_COUNTRY_GROUPS: &[(&str, &str, &str)] = &[
    ("HK", "香港负载", "香港故障转移"),
    ("JP", "日本负载", "日本故障转移"),
    ("SG", "新加坡负载", "新加坡故障转移"),
    ("US", "美国负载", "美国故障转移"),
    ("TW", "台湾负载", "台湾故障转移"),
];
const MIHOMO_OTHER_COUNTRY_GROUP: &str = "其他地区负载";
const COUNTRY_NAMES_ZH: &[(&str, &str)] = &[
    ("AD", "安道尔"),
    ("AE", "阿联酋"),
    ("AF", "阿富汗"),
    ("AG", "安提瓜和巴布达"),
    ("AI", "安圭拉"),
    ("AL", "阿尔巴尼亚"),
    ("AM", "亚美尼亚"),
    ("AO", "安哥拉"),
    ("AQ", "南极洲"),
    ("AR", "阿根廷"),
    ("AS", "美属萨摩亚"),
    ("AT", "奥地利"),
    ("AU", "澳大利亚"),
    ("AW", "阿鲁巴"),
    ("AX", "奥兰群岛"),
    ("AZ", "阿塞拜疆"),
    ("BA", "波斯尼亚和黑塞哥维那"),
    ("BB", "巴巴多斯"),
    ("BD", "孟加拉国"),
    ("BE", "比利时"),
    ("BF", "布基纳法索"),
    ("BG", "保加利亚"),
    ("BH", "巴林"),
    ("BI", "布隆迪"),
    ("BJ", "贝宁"),
    ("BL", "圣巴泰勒米"),
    ("BM", "百慕大"),
    ("BN", "文莱"),
    ("BO", "玻利维亚"),
    ("BQ", "荷兰加勒比区"),
    ("BR", "巴西"),
    ("BS", "巴哈马"),
    ("BT", "不丹"),
    ("BV", "布韦岛"),
    ("BW", "博茨瓦纳"),
    ("BY", "白俄罗斯"),
    ("BZ", "伯利兹"),
    ("CA", "加拿大"),
    ("CC", "科科斯群岛"),
    ("CD", "刚果（金）"),
    ("CF", "中非共和国"),
    ("CG", "刚果（布）"),
    ("CH", "瑞士"),
    ("CI", "科特迪瓦"),
    ("CK", "库克群岛"),
    ("CL", "智利"),
    ("CM", "喀麦隆"),
    ("CN", "中国"),
    ("CO", "哥伦比亚"),
    ("CR", "哥斯达黎加"),
    ("CU", "古巴"),
    ("CV", "佛得角"),
    ("CW", "库拉索"),
    ("CX", "圣诞岛"),
    ("CY", "塞浦路斯"),
    ("CZ", "捷克"),
    ("DE", "德国"),
    ("DJ", "吉布提"),
    ("DK", "丹麦"),
    ("DM", "多米尼克"),
    ("DO", "多米尼加共和国"),
    ("DZ", "阿尔及利亚"),
    ("EC", "厄瓜多尔"),
    ("EE", "爱沙尼亚"),
    ("EG", "埃及"),
    ("EH", "西撒哈拉"),
    ("ER", "厄立特里亚"),
    ("ES", "西班牙"),
    ("ET", "埃塞俄比亚"),
    ("FI", "芬兰"),
    ("FJ", "斐济"),
    ("FK", "福克兰群岛"),
    ("FM", "密克罗尼西亚联邦"),
    ("FO", "法罗群岛"),
    ("FR", "法国"),
    ("GA", "加蓬"),
    ("GB", "英国"),
    ("GD", "格林纳达"),
    ("GE", "格鲁吉亚"),
    ("GF", "法属圭亚那"),
    ("GG", "根西岛"),
    ("GH", "加纳"),
    ("GI", "直布罗陀"),
    ("GL", "格陵兰"),
    ("GM", "冈比亚"),
    ("GN", "几内亚"),
    ("GP", "瓜德罗普"),
    ("GQ", "赤道几内亚"),
    ("GR", "希腊"),
    ("GS", "南乔治亚和南桑威奇群岛"),
    ("GT", "危地马拉"),
    ("GU", "关岛"),
    ("GW", "几内亚比绍"),
    ("GY", "圭亚那"),
    ("HK", "香港"),
    ("HM", "赫德岛和麦克唐纳群岛"),
    ("HN", "洪都拉斯"),
    ("HR", "克罗地亚"),
    ("HT", "海地"),
    ("HU", "匈牙利"),
    ("ID", "印度尼西亚"),
    ("IE", "爱尔兰"),
    ("IL", "以色列"),
    ("IM", "马恩岛"),
    ("IN", "印度"),
    ("IO", "英属印度洋领地"),
    ("IQ", "伊拉克"),
    ("IR", "伊朗"),
    ("IS", "冰岛"),
    ("IT", "意大利"),
    ("JE", "泽西岛"),
    ("JM", "牙买加"),
    ("JO", "约旦"),
    ("JP", "日本"),
    ("KE", "肯尼亚"),
    ("KG", "吉尔吉斯斯坦"),
    ("KH", "柬埔寨"),
    ("KI", "基里巴斯"),
    ("KM", "科摩罗"),
    ("KN", "圣基茨和尼维斯"),
    ("KP", "朝鲜"),
    ("KR", "韩国"),
    ("KW", "科威特"),
    ("KY", "开曼群岛"),
    ("KZ", "哈萨克斯坦"),
    ("LA", "老挝"),
    ("LB", "黎巴嫩"),
    ("LC", "圣卢西亚"),
    ("LI", "列支敦士登"),
    ("LK", "斯里兰卡"),
    ("LR", "利比里亚"),
    ("LS", "莱索托"),
    ("LT", "立陶宛"),
    ("LU", "卢森堡"),
    ("LV", "拉脱维亚"),
    ("LY", "利比亚"),
    ("MA", "摩洛哥"),
    ("MC", "摩纳哥"),
    ("MD", "摩尔多瓦"),
    ("ME", "黑山"),
    ("MF", "法属圣马丁"),
    ("MG", "马达加斯加"),
    ("MH", "马绍尔群岛"),
    ("MK", "北马其顿"),
    ("ML", "马里"),
    ("MM", "缅甸"),
    ("MN", "蒙古"),
    ("MO", "澳门"),
    ("MP", "北马里亚纳群岛"),
    ("MQ", "马提尼克"),
    ("MR", "毛里塔尼亚"),
    ("MS", "蒙特塞拉特"),
    ("MT", "马耳他"),
    ("MU", "毛里求斯"),
    ("MV", "马尔代夫"),
    ("MW", "马拉维"),
    ("MX", "墨西哥"),
    ("MY", "马来西亚"),
    ("MZ", "莫桑比克"),
    ("NA", "纳米比亚"),
    ("NC", "新喀里多尼亚"),
    ("NE", "尼日尔"),
    ("NF", "诺福克岛"),
    ("NG", "尼日利亚"),
    ("NI", "尼加拉瓜"),
    ("NL", "荷兰"),
    ("NO", "挪威"),
    ("NP", "尼泊尔"),
    ("NR", "瑙鲁"),
    ("NU", "纽埃"),
    ("NZ", "新西兰"),
    ("OM", "阿曼"),
    ("PA", "巴拿马"),
    ("PE", "秘鲁"),
    ("PF", "法属波利尼西亚"),
    ("PG", "巴布亚新几内亚"),
    ("PH", "菲律宾"),
    ("PK", "巴基斯坦"),
    ("PL", "波兰"),
    ("PM", "圣皮埃尔和密克隆"),
    ("PN", "皮特凯恩群岛"),
    ("PR", "波多黎各"),
    ("PS", "巴勒斯坦"),
    ("PT", "葡萄牙"),
    ("PW", "帕劳"),
    ("PY", "巴拉圭"),
    ("QA", "卡塔尔"),
    ("RE", "留尼汪"),
    ("RO", "罗马尼亚"),
    ("RS", "塞尔维亚"),
    ("RU", "俄罗斯"),
    ("RW", "卢旺达"),
    ("SA", "沙特阿拉伯"),
    ("SB", "所罗门群岛"),
    ("SC", "塞舌尔"),
    ("SD", "苏丹"),
    ("SE", "瑞典"),
    ("SG", "新加坡"),
    ("SH", "圣赫勒拿"),
    ("SI", "斯洛文尼亚"),
    ("SJ", "斯瓦尔巴和扬马延"),
    ("SK", "斯洛伐克"),
    ("SL", "塞拉利昂"),
    ("SM", "圣马力诺"),
    ("SN", "塞内加尔"),
    ("SO", "索马里"),
    ("SR", "苏里南"),
    ("SS", "南苏丹"),
    ("ST", "圣多美和普林西比"),
    ("SV", "萨尔瓦多"),
    ("SX", "荷属圣马丁"),
    ("SY", "叙利亚"),
    ("SZ", "斯威士兰"),
    ("TC", "特克斯和凯科斯群岛"),
    ("TD", "乍得"),
    ("TF", "法属南部领地"),
    ("TG", "多哥"),
    ("TH", "泰国"),
    ("TJ", "塔吉克斯坦"),
    ("TK", "托克劳"),
    ("TL", "东帝汶"),
    ("TM", "土库曼斯坦"),
    ("TN", "突尼斯"),
    ("TO", "汤加"),
    ("TR", "土耳其"),
    ("TT", "特立尼达和多巴哥"),
    ("TV", "图瓦卢"),
    ("TW", "台湾"),
    ("TZ", "坦桑尼亚"),
    ("UA", "乌克兰"),
    ("UG", "乌干达"),
    ("UM", "美国本土外小岛屿"),
    ("US", "美国"),
    ("UY", "乌拉圭"),
    ("UZ", "乌兹别克斯坦"),
    ("VA", "梵蒂冈"),
    ("VC", "圣文森特和格林纳丁斯"),
    ("VE", "委内瑞拉"),
    ("VG", "英属维尔京群岛"),
    ("VI", "美属维尔京群岛"),
    ("VN", "越南"),
    ("VU", "瓦努阿图"),
    ("WF", "瓦利斯和富图纳"),
    ("WS", "萨摩亚"),
    ("YE", "也门"),
    ("YT", "马约特"),
    ("ZA", "南非"),
    ("ZM", "赞比亚"),
    ("ZW", "津巴布韦"),
    ("AP", "亚太地区"),
    ("EU", "欧盟"),
    ("UK", "英国"),
    ("XK", "科索沃"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportMode {
    Strict,
    BestEffort,
}

struct ExportNodeMetadata<'a> {
    countries: &'a HashMap<i64, String>,
    risks: &'a HashMap<i64, NodeIpProbeRecord>,
}

impl ExportMode {
    fn as_cache_key(self) -> &'static str {
        match self {
            ExportMode::Strict => "strict",
            ExportMode::BestEffort => "best_effort",
        }
    }
}

pub async fn export_subscription(
    state: &AppState,
    token: &str,
    target: Option<&str>,
    mode: Option<&str>,
    user_agent: Option<&str>,
    headers: &HeaderMap,
    peer_ip: Option<IpAddr>,
) -> Result<Response, AppError> {
    let settings = settings_service::load_settings(state).await?;
    ensure_public_export_not_limited(state, headers, peer_ip, &settings).await?;

    let subscription_record = subscription_repo::find_by_token(&state.db, token)
        .await?
        .ok_or_else(|| AppError::NotFound("subscription not found".to_string()))?;
    if subscription_record.enabled == 0 || subscription_record_is_expired(&subscription_record) {
        return Err(AppError::NotFound("subscription not found".to_string()));
    }

    let detected_target = detect_export_target(user_agent);
    let export_target = target
        .or(detected_target)
        .or(subscription_record.default_client.as_deref())
        .unwrap_or("xray")
        .to_string();
    let export_mode = resolve_export_mode(target, mode, detected_target)?;
    let canonical_target = canonical_export_target(&export_target)?;
    let template_target = template_target(&export_target, canonical_target).to_string();
    let cache_key = public_export_cache_key(
        subscription_record.id,
        &subscription_record.token,
        &subscription_record.updated_at,
        subscription_record.expires_at.as_deref(),
        &export_target,
        export_mode,
        subscription_record.include_rules != 0,
    );
    let cache_ttl = Duration::from_secs(settings.public_export_cache_ttl_seconds as u64);
    if !cache_ttl.is_zero()
        && let Some(cached) = state.get_public_export_cache(&cache_key, cache_ttl).await
    {
        return Ok(response_from_cached_export(cached));
    }

    let subscription = subscription_service::get_subscription_by_token(state, token).await?;
    if !subscription.enabled || subscription_service::subscription_is_expired(&subscription) {
        return Err(AppError::NotFound("subscription not found".to_string()));
    }

    let response = export_subscription_view_with_resolved_target(
        state,
        subscription,
        export_target,
        export_mode,
        canonical_target,
        &template_target,
        &settings,
    )
    .await?;
    if cache_ttl.is_zero() {
        Ok(response)
    } else {
        cache_public_export_response(state, cache_key, response).await
    }
}

pub async fn export_subscription_by_id(
    state: &AppState,
    id: i64,
    target: Option<&str>,
    mode: Option<&str>,
    user_agent: Option<&str>,
) -> Result<Response, AppError> {
    let response = subscription_service::get_subscription(state, id).await?;
    let subscription = response.data;
    if !subscription.enabled || subscription_service::subscription_is_expired(&subscription) {
        return Err(AppError::NotFound("subscription not found".to_string()));
    }

    export_subscription_view(state, subscription, target, mode, user_agent).await
}

async fn ensure_public_export_not_limited(
    state: &AppState,
    headers: &HeaderMap,
    peer_ip: Option<IpAddr>,
    settings: &AppSettingsView,
) -> Result<(), AppError> {
    let ip = auth_service::request_rate_limit_ip(
        headers,
        peer_ip,
        state.config.security.trust_proxy_headers,
    );
    state
        .check_rate_limit(
            &format!("public-export:ip:{ip}"),
            settings.public_export_ip_limit_per_minute as u32,
            PUBLIC_EXPORT_RATE_WINDOW,
        )
        .await
        .map_err(|message| AppError::TooManyRequests(message.to_string()))?;
    state
        .check_rate_limit(
            "public-export:global",
            settings.public_export_global_limit_per_minute as u32,
            PUBLIC_EXPORT_RATE_WINDOW,
        )
        .await
        .map_err(|message| AppError::TooManyRequests(message.to_string()))?;
    Ok(())
}

fn public_export_cache_key(
    subscription_id: i64,
    token: &str,
    updated_at: &str,
    expires_at: Option<&str>,
    export_target: &str,
    export_mode: ExportMode,
    include_rules: bool,
) -> String {
    format!(
        "public-export:v2:{subscription_id}:{token}:{updated_at}:{}:{export_target}:{}:{include_rules}",
        expires_at.unwrap_or_default(),
        export_mode.as_cache_key()
    )
}

fn subscription_record_is_expired(
    record: &crate::domain::subscription::SubscriptionRecord,
) -> bool {
    record
        .expires_at
        .as_deref()
        .and_then(|value| OffsetDateTime::parse(value, &Rfc3339).ok())
        .is_some_and(|expires_at| expires_at <= OffsetDateTime::now_utc())
}

fn response_from_cached_export(entry: PublicExportCacheEntry) -> Response {
    let mut response = Response::new(Body::from(entry.body));
    *response.status_mut() = entry.status;
    *response.headers_mut() = entry.headers;
    response
}

async fn cache_public_export_response(
    state: &AppState,
    key: String,
    response: Response,
) -> Result<Response, AppError> {
    let (parts, body) = response.into_parts();
    let bytes = to_bytes(body, PUBLIC_EXPORT_CACHE_BODY_LIMIT)
        .await
        .map_err(|_| AppError::Internal)?;
    state
        .set_public_export_cache(
            key,
            PublicExportCacheEntry {
                status: parts.status,
                headers: parts.headers.clone(),
                body: bytes.clone(),
                cached_at: Instant::now(),
            },
        )
        .await;

    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = parts.status;
    *response.headers_mut() = parts.headers;
    Ok(response)
}

async fn export_subscription_view(
    state: &AppState,
    subscription: SubscriptionView,
    target: Option<&str>,
    mode: Option<&str>,
    user_agent: Option<&str>,
) -> Result<Response, AppError> {
    let settings = settings_service::load_settings(state).await?;
    let detected_target = detect_export_target(user_agent);
    let export_target = target
        .or(detected_target)
        .or(subscription.default_client.as_deref())
        .unwrap_or("xray")
        .to_string();
    let export_mode = resolve_export_mode(target, mode, detected_target)?;
    let canonical_target = canonical_export_target(&export_target)?;
    let template_target = template_target(&export_target, canonical_target).to_string();
    export_subscription_view_with_resolved_target(
        state,
        subscription,
        export_target,
        export_mode,
        canonical_target,
        &template_target,
        &settings,
    )
    .await
}

async fn export_subscription_view_with_resolved_target(
    state: &AppState,
    mut subscription: SubscriptionView,
    export_target: String,
    export_mode: ExportMode,
    canonical_target: &'static str,
    template_target: &str,
    settings: &AppSettingsView,
) -> Result<Response, AppError> {
    let candidate_node_ids = subscription
        .nodes
        .iter()
        .map(|node| node.id)
        .collect::<Vec<_>>();
    let node_risks =
        node_ip_probe_repo::records_by_node_ids(&state.db, &candidate_node_ids).await?;
    subscription.nodes.retain(|node| {
        node.enabled
            && !node.upstream_missing
            && node_ip_probe_repo::risk_allows_export(
                node_risks.get(&node.id),
                settings.risk_enforcement_enabled,
            )
    });
    subscription.node_ids = subscription.nodes.iter().map(|node| node.id).collect();
    sort_subscription_nodes_for_export(state, &mut subscription).await?;
    let template =
        load_export_template(&state.db, subscription.template_id, template_target).await?;
    let node_countries = if matches!(
        canonical_target,
        "mihomo" | "mellow" | "quanx" | "shadowrocket-profile"
    ) {
        node_ip_probe_repo::countries_by_node_ids(&state.db, &subscription.node_ids).await?
    } else {
        HashMap::new()
    };

    let include_rules = subscription.include_rules;
    let response = match canonical_target {
        "xray" => export_xray_bundle(subscription, export_mode),
        "uri-bundle" => export_uri_bundle(subscription, export_mode, &export_target),
        "sssub" => export_sssub(subscription, export_mode, template.as_ref()),
        "ssd" => export_ssd(subscription, export_mode, template.as_ref()),
        "mihomo" => export_mihomo(
            subscription,
            export_mode,
            template.as_ref(),
            template_target,
            &ExportNodeMetadata {
                countries: &node_countries,
                risks: &node_risks,
            },
            settings.mihomo_country_load_min_nodes as usize,
            settings.mihomo_country_fallback_min_nodes as usize,
        ),
        "surge" => export_surge(subscription, export_mode, template.as_ref()),
        "sing-box" => export_sing_box(subscription, export_mode, template.as_ref()),
        "quanx" => export_quantumult_x(
            subscription,
            export_mode,
            template.as_ref(),
            &ExportNodeMetadata {
                countries: &node_countries,
                risks: &node_risks,
            },
        ),
        "shadowrocket-profile" => export_shadowrocket_profile(
            subscription,
            export_mode,
            template.as_ref(),
            &ExportNodeMetadata {
                countries: &node_countries,
                risks: &node_risks,
            },
        ),
        "quan" => {
            export_legacy_client_profile(subscription, export_mode, "Quantumult", template.as_ref())
        }
        "loon" => {
            export_legacy_client_profile(subscription, export_mode, "Loon", template.as_ref())
        }
        "surfboard" => {
            export_legacy_client_profile(subscription, export_mode, "Surfboard", template.as_ref())
        }
        "mellow" => export_mihomo(
            subscription,
            export_mode,
            template.as_ref(),
            template_target,
            &ExportNodeMetadata {
                countries: &node_countries,
                risks: &node_risks,
            },
            settings.mihomo_country_load_min_nodes as usize,
            settings.mihomo_country_fallback_min_nodes as usize,
        ),
        _ => Err(AppError::Internal),
    }?;
    if include_rules {
        return Ok(response);
    }
    let (mut parts, body) = response.into_parts();
    let bytes = to_bytes(body, usize::MAX)
        .await
        .map_err(|_| AppError::Internal)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| AppError::Internal)?;
    let nodes = nodes_only_export(text, canonical_target)?;
    parts.headers.remove(header::CONTENT_LENGTH);
    Ok(Response::from_parts(parts, Body::from(nodes)))
}

fn nodes_only_export(text: &str, target: &str) -> Result<String, AppError> {
    match target {
        "mihomo" | "mellow" => {
            let root: Value = serde_yaml::from_str(text).map_err(|_| AppError::Internal)?;
            let mut nodes = Mapping::new();
            for key in ["proxies", "proxy-providers", "proxy-groups"] {
                if let Some(value) = root.get(key) {
                    nodes.insert(Value::String(key.into()), value.clone());
                }
            }
            // Keep routing tied to the selected group without importing website rules.
            if mihomo_proxy_group_exists(&nodes, "节点选择") {
                nodes.insert(
                    Value::String("rules".into()),
                    Value::Sequence(vec![Value::String("MATCH,节点选择".into())]),
                );
            }
            ensure_mihomo_match_rule(&mut nodes);
            nodes.insert(Value::String("mode".into()), Value::String("rule".into()));
            serde_yaml::to_string(&nodes).map_err(|_| AppError::Internal)
        }
        "sing-box" => {
            let mut root: serde_json::Value =
                serde_json::from_str(text).map_err(|_| AppError::Internal)?;
            let outbounds = root["outbounds"].as_array_mut().ok_or(AppError::Internal)?;
            serde_json::to_string_pretty(&json!({"outbounds": outbounds}))
                .map_err(|_| AppError::Internal)
        }
        "surge" | "quanx" | "shadowrocket-profile" | "quan" | "loon" | "surfboard" => {
            let mut keep = false;
            let mut lines = Vec::new();
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('[') && trimmed.ends_with(']') {
                    let section = trimmed.to_ascii_lowercase();
                    keep = matches!(
                        section.as_str(),
                        "[proxy]" | "[server]" | "[server_local]" | "[proxy group]" | "[policy]"
                    ) || section.starts_with("[wireguard ");
                }
                if keep {
                    lines.push(line);
                }
            }
            Ok(format!("{}\n", lines.join("\n")))
        }
        _ => Ok(text.to_string()),
    }
}

async fn sort_subscription_nodes_for_export(
    state: &AppState,
    subscription: &mut SubscriptionView,
) -> Result<(), AppError> {
    let groups = group_repo::list(&state.db, GroupTable::Node).await?;
    let group_order_by_id = groups
        .iter()
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

fn export_sssub(
    subscription: SubscriptionView,
    mode: ExportMode,
    template: Option<&TemplateRecord>,
) -> Result<Response, AppError> {
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

    let mut config = parse_json_template(template, "sssub")?;
    let root = config
        .as_object_mut()
        .ok_or_else(|| AppError::BadRequest("sssub template must be a JSON object".to_string()))?;
    root.entry("version".to_string())
        .or_insert_with(|| json!(1));
    root.insert("remarks".to_string(), json!(subscription.name));
    root.insert("servers".to_string(), serde_json::Value::Array(servers));
    let body = serde_json::to_string_pretty(&config).map_err(|_| AppError::Internal)?;

    text_response(
        body,
        "application/json; charset=utf-8",
        &format!("{}.json", subscription.name),
        mode,
        filtered_count,
    )
}

fn export_ssd(
    subscription: SubscriptionView,
    mode: ExportMode,
    template: Option<&TemplateRecord>,
) -> Result<Response, AppError> {
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

    let mut config = parse_json_template(template, "ssd")?;
    let root = config
        .as_object_mut()
        .ok_or_else(|| AppError::BadRequest("ssd template must be a JSON object".to_string()))?;
    root.insert("airport".to_string(), json!(subscription.name));
    root.entry("port".to_string()).or_insert_with(|| json!(0));
    root.entry("encryption".to_string())
        .or_insert_with(|| json!(""));
    root.entry("password".to_string())
        .or_insert_with(|| json!(""));
    root.entry("traffic_used".to_string())
        .or_insert_with(|| json!(0));
    root.entry("traffic_total".to_string())
        .or_insert_with(|| json!(0));
    root.entry("expiry".to_string()).or_insert_with(|| json!(0));
    root.entry("url".to_string()).or_insert_with(|| json!(""));
    root.insert("servers".to_string(), serde_json::Value::Array(servers));
    let body = serde_json::to_string_pretty(&config).map_err(|_| AppError::Internal)?;

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
    metadata: &ExportNodeMetadata<'_>,
    country_load_min_nodes: usize,
    country_fallback_min_nodes: usize,
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
        dedupe_mihomo_proxy_names(&mut root);
        root.remove(Value::String("x-sublinkx-upstream-template".to_string()));
        let yaml = serde_yaml::to_string(&root).map_err(|_| AppError::Internal)?;
        return text_response(
            yaml,
            "application/yaml; charset=utf-8",
            &format!("{}.yaml", subscription.name),
            mode,
            0,
        );
    }

    let mut root = parse_yaml_template(template, target_name)?;
    dedupe_mihomo_proxy_names(&mut root);
    compact_builtin_mihomo_policy_groups(&mut root, template);

    let mut used_proxy_names = collect_mihomo_proxy_names(&root);
    let mut proxy_items = Vec::with_capacity(subscription.nodes.len());
    let mut proxy_names = Vec::with_capacity(subscription.nodes.len());
    let mut proxy_names_by_node_id = HashMap::with_capacity(subscription.nodes.len());
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

        let mut rendered = render_mihomo_proxy(node)?;
        let proxy_name = unique_mihomo_proxy_name(&node.name, &mut used_proxy_names);
        rendered.insert(
            Value::String("name".to_string()),
            Value::String(proxy_name.clone()),
        );
        proxy_names_by_node_id.insert(node.id, proxy_name.clone());
        proxy_names.push(proxy_name);
        proxy_items.push(Value::Mapping(rendered));
    }

    if proxy_items.is_empty() {
        return Err(AppError::BadRequest(
            "no compatible nodes available for mihomo export".to_string(),
        ));
    }

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
    expand_mihomo_include_all_proxy_groups(&mut root, &proxy_names);
    ensure_mihomo_default_proxy_groups(&mut root, &proxy_names);
    ensure_mihomo_country_load_balance_groups(
        &mut root,
        metadata.countries,
        &proxy_names_by_node_id,
        country_load_min_nodes,
        country_fallback_min_nodes,
    );
    ensure_mihomo_fraud_groups(
        &mut root,
        metadata.countries,
        metadata.risks,
        &proxy_names_by_node_id,
    );
    ensure_mihomo_match_rule(&mut root);
    localize_mihomo_legacy_group_names(&mut root);

    let yaml = serialize_mihomo_yaml(&root, template)?;
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
    // sing-box rejects unknown top-level metadata fields during config validation.
    root.remove("remarks");
    root.remove("title");
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
    template: Option<&TemplateRecord>,
    metadata: &ExportNodeMetadata<'_>,
) -> Result<Response, AppError> {
    let mut lines = Vec::new();
    let mut proxy_names_by_node_id = HashMap::with_capacity(subscription.nodes.len());
    let mut used_names = HashSet::with_capacity(subscription.nodes.len());
    let mut filtered_count = 0usize;

    for node in &subscription.nodes {
        let tag = unique_line_proxy_name(node, &mut used_names);
        match render_quantumult_x_proxy_with_tag(node, &tag) {
            Ok(line) => {
                proxy_names_by_node_id.insert(node.id, tag);
                lines.push(line);
            }
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

    let template_content = match template {
        Some(template)
            if template.is_builtin != 0 && template.name == BUILTIN_QUANX_TEMPLATE_NAME =>
        {
            QUANX_DEFAULT_TEMPLATE
        }
        Some(template) => &template.content,
        None => QUANX_DEFAULT_TEMPLATE,
    };
    let mut ini = parse_surge_template(template_content);
    append_lines_to_section(&mut ini.sections, "server_local", lines);
    ensure_section(
        &mut ini.sections,
        "filter_local",
        vec![
            "geoip, cn, direct".to_string(),
            format!("final, {QUANX_GROUP_PROXY}"),
        ],
    );
    let groups =
        collect_risk_policy_groups(metadata.countries, metadata.risks, &proxy_names_by_node_id);
    configure_ini_risk_policy_groups(&mut ini.sections, IniPolicyFormat::QuantumultX, &groups);
    let mut body = render_surge_ini(ini);
    if !body.ends_with('\n') {
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

fn export_shadowrocket_profile(
    subscription: SubscriptionView,
    mode: ExportMode,
    template: Option<&TemplateRecord>,
    metadata: &ExportNodeMetadata<'_>,
) -> Result<Response, AppError> {
    let mut proxy_lines = Vec::with_capacity(subscription.nodes.len());
    let mut proxy_names_by_node_id = HashMap::with_capacity(subscription.nodes.len());
    let mut profile_proxy_names = Vec::with_capacity(subscription.nodes.len());
    let mut wireguard_sections = Vec::new();
    let mut used_names = HashSet::with_capacity(subscription.nodes.len());
    let mut filtered_count = 0usize;

    for node in &subscription.nodes {
        if !is_surge_supported(node) {
            if matches!(mode, ExportMode::BestEffort) {
                filtered_count += 1;
                continue;
            }
            return Err(AppError::BadRequest(format!(
                "node '{}' is not supported by shadowrocket profile export",
                node.name
            )));
        }

        let name = unique_line_proxy_name(node, &mut used_names);
        match render_surge_proxy_with_name(node, name) {
            Ok(rendered) => {
                proxy_names_by_node_id.insert(node.id, rendered.name.clone());
                profile_proxy_names.push(rendered.name.clone());
                proxy_lines.push(rendered.proxy_line);
                if let Some(section) = rendered.wireguard_section {
                    wireguard_sections.push(section);
                }
            }
            Err(error) if matches!(mode, ExportMode::BestEffort) => {
                tracing::debug!(
                    ?error,
                    node_id = node.id,
                    "filtered unsupported shadowrocket profile node"
                );
                filtered_count += 1;
            }
            Err(error) => return Err(error),
        }
    }

    if proxy_lines.is_empty() {
        return Err(AppError::BadRequest(
            "no compatible nodes available for shadowrocket profile export".to_string(),
        ));
    }

    let mut ini = if let Some(template) = template {
        parse_surge_template(&template.content)
    } else {
        parse_surge_template(
            "[General]\nloglevel = notify\n\n\
[Proxy]\n\n\
[Proxy Group]\n代理选择 = select, 手动选择, 自动选择, direct\n\
手动选择 = select\n\
自动选择 = url-test, url=https://cp.cloudflare.com/generate_204, interval=600, tolerance=0\n\n\
[Rule]\nFINAL,代理选择\n",
        )
    };
    append_lines_to_section(&mut ini.sections, "Proxy", proxy_lines);
    configure_shadowrocket_base_policy_groups(&mut ini.sections, &profile_proxy_names);
    let groups =
        collect_risk_policy_groups(metadata.countries, metadata.risks, &proxy_names_by_node_id);
    configure_ini_risk_policy_groups(&mut ini.sections, IniPolicyFormat::Shadowrocket, &groups);
    ensure_section(
        &mut ini.sections,
        "Rule",
        vec!["GEOIP,CN,DIRECT".to_string(), "FINAL,代理选择".to_string()],
    );
    ini.trailing_raw_sections.extend(wireguard_sections);
    let mut body = render_surge_ini(ini);
    if !body.ends_with('\n') {
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

fn export_legacy_client_profile(
    subscription: SubscriptionView,
    mode: ExportMode,
    client_name: &str,
    template: Option<&TemplateRecord>,
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

    let mut body = if let Some(template) = template {
        let mut ini = parse_surge_template(&template.content);
        let proxy_section = if client_name == "Quantumult" {
            "SERVER"
        } else {
            "Proxy"
        };
        append_lines_to_section(&mut ini.sections, proxy_section, proxy_lines);
        ini.trailing_raw_sections.extend(wireguard_sections);
        render_surge_ini(ini)
    } else {
        let mut generated = format!(
            "#!MANAGED-CONFIG {} interval=86400 strict=false\n# Profile: {}\n# Client: {}\n\n[General]\nloglevel = notify\n\n[Proxy]\n{}\n\n[Proxy Group]\nProxy = select, {}, DIRECT\n\n[Rule]\nFINAL,Proxy\n",
            subscription.name,
            subscription.name,
            client_name,
            proxy_lines.join("\n"),
            proxy_names.join(", ")
        );

        if !wireguard_sections.is_empty() {
            generated.push('\n');
            generated.push_str(&wireguard_sections.join("\n\n"));
            generated.push('\n');
        }
        generated
    };
    if !body.ends_with('\n') {
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
            insert_optional_json_bool(&mut map, "udp", &node.settings, "udp");
            insert_optional_json_string(&mut map, "plugin", &node.settings, "plugin");
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
            if let Some(fp) = node.settings.get("fp").and_then(|v| v.as_str())
                && !fp.is_empty()
            {
                map.insert(
                    Value::String("client-fingerprint".to_string()),
                    Value::String(fp.to_string()),
                );
            }
            if let Some(insecure) = node.settings.get("insecure").and_then(json_to_bool) {
                map.insert(
                    Value::String("skip-cert-verify".to_string()),
                    Value::Bool(insecure),
                );
            }
            insert_optional_json_bool(&mut map, "udp", &node.settings, "udp");
            if let Some(alpn) = node.settings.get("alpn").and_then(|v| v.as_str())
                && !alpn.is_empty()
            {
                map.insert(
                    Value::String("alpn".to_string()),
                    Value::Sequence(split_csv(alpn)),
                );
            }
            if node
                .settings
                .get("net")
                .and_then(|v| v.as_str())
                .is_some_and(|network| network.eq_ignore_ascii_case("grpc"))
                && let Some(service_name) = node
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
                && let Some(service_name) = node
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
            insert_optional_json_string(
                &mut map,
                "name-cert-verify",
                &node.settings,
                "name-cert-verify",
            );
            for key in ["shadow-tls-opts", "restls-opts", "jls-opts"] {
                if let Some(raw) = node.settings.get(key).and_then(|value| value.as_str())
                    && let Ok(value) = serde_json::from_str::<serde_json::Value>(raw)
                    && let Ok(value) = serde_yaml::to_value(value)
                {
                    map.insert(Value::String(key.to_string()), value);
                }
            }
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
            if !insert_serialized_json_mapping(&mut map, "ws-opts", &node.settings, "ws-opts")
                && let Some(path) = node.settings.get("path").and_then(|v| v.as_str())
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
            if !insert_serialized_json_mapping(&mut map, "grpc-opts", &node.settings, "grpc-opts")
                && node
                    .settings
                    .get("type")
                    .and_then(|v| v.as_str())
                    .is_some_and(|network| network.eq_ignore_ascii_case("grpc"))
                && let Some(service_name) = node
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
            map.insert(Value::String("tls".to_string()), Value::Bool(true));
            if let Some(sni) = node.settings.get("sni").and_then(|v| v.as_str())
                && !sni.is_empty()
            {
                map.insert(
                    Value::String("sni".to_string()),
                    Value::String(sni.to_string()),
                );
            }
            insert_optional_json_string(&mut map, "client-fingerprint", &node.settings, "fp");
            insert_optional_json_string(
                &mut map,
                "fingerprint",
                &node.settings,
                "certificate-fingerprint",
            );
            insert_optional_json_bool(&mut map, "skip-cert-verify", &node.settings, "insecure");
            insert_optional_json_bool(&mut map, "udp", &node.settings, "udp");
            insert_optional_json_string(
                &mut map,
                "name-cert-verify",
                &node.settings,
                "name-cert-verify",
            );
            insert_optional_json_string(&mut map, "ip-version", &node.settings, "ip-version");
            insert_optional_json_bool(&mut map, "tfo", &node.settings, "tfo");
            insert_optional_json_bool(&mut map, "mptcp", &node.settings, "mptcp");
            for key in [
                "reality-opts",
                "ech-opts",
                "shadow-tls-opts",
                "restls-opts",
                "jls-opts",
                "ss-opts",
                "smux",
            ] {
                insert_serialized_json_mapping(&mut map, key, &node.settings, key);
            }
            if map.get(Value::String("reality-opts".to_string())).is_none()
                && let Some(public_key) = node.settings.get("pbk").and_then(|v| v.as_str())
                && !public_key.is_empty()
            {
                let mut reality = Mapping::new();
                reality.insert(
                    Value::String("public-key".to_string()),
                    Value::String(public_key.to_string()),
                );
                if let Some(short_id) = node.settings.get("sid").and_then(|v| v.as_str())
                    && !short_id.is_empty()
                {
                    reality.insert(
                        Value::String("short-id".to_string()),
                        Value::String(short_id.to_string()),
                    );
                }
                map.insert(
                    Value::String("reality-opts".to_string()),
                    Value::Mapping(reality),
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
            if let Some(token) = node.settings.get("token").and_then(|v| v.as_str())
                && !token.is_empty()
            {
                map.insert(
                    Value::String("token".to_string()),
                    Value::String(token.to_string()),
                );
            } else {
                insert_json_string(&mut map, "uuid", &node.settings, "uuid")?;
                insert_json_string(&mut map, "password", &node.settings, "password")?;
            }
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
            for key in ["ip", "bbr-profile", "name-cert-verify"] {
                insert_optional_json_string(&mut map, key, &node.settings, key);
            }
            for key in [
                "heartbeat-interval",
                "request-timeout",
                "max-udp-relay-packet-size",
                "max-open-streams",
            ] {
                insert_optional_json_i64(&mut map, key, &node.settings, key);
            }
            for key in ["disable-sni", "reduce-rtt", "fast-open"] {
                insert_optional_json_bool(&mut map, key, &node.settings, key);
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

            if let Some(allowed_ips) = node.settings.get("allowed-ips").and_then(|v| v.as_str())
                && !allowed_ips.is_empty()
            {
                map.insert(
                    Value::String("allowed-ips".to_string()),
                    Value::Sequence(split_csv(allowed_ips)),
                );
            }
            if let Some(reserved) = node.settings.get("reserved").and_then(|v| v.as_str())
                && !reserved.is_empty()
            {
                let value = if reserved.contains(',') {
                    Value::Sequence(split_csv_scalars(reserved))
                } else {
                    Value::String(reserved.to_string())
                };
                map.insert(Value::String("reserved".to_string()), value);
            }
            insert_optional_json_i64(
                &mut map,
                "persistent-keepalive",
                &node.settings,
                "persistent-keepalive",
            );
            insert_optional_json_bool(
                &mut map,
                "remote-dns-resolve",
                &node.settings,
                "remote-dns-resolve",
            );
            if let Some(dns) = node.settings.get("dns").and_then(|v| v.as_str())
                && !dns.is_empty()
            {
                map.insert(
                    Value::String("dns".to_string()),
                    Value::Sequence(split_csv(dns)),
                );
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

fn resolve_export_mode(
    requested_target: Option<&str>,
    requested_mode: Option<&str>,
    detected_target: Option<&str>,
) -> Result<ExportMode, AppError> {
    let requested_mode = parse_export_mode(requested_mode)?;
    if requested_target.is_none() && detected_target.is_some() {
        Ok(ExportMode::BestEffort)
    } else {
        Ok(requested_mode)
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
                let server_ports = normalize_sing_box_server_ports(ports).ok_or_else(|| {
                    AppError::BadRequest(format!(
                        "node '{}' has an invalid hysteria2 port range",
                        node.name
                    ))
                })?;
                outbound["server_ports"] = json!(server_ports);
            }
            if let Some(up) = node.settings.get("up").and_then(json_to_i64) {
                outbound["up_mbps"] = json!(up);
            }
            if let Some(down) = node.settings.get("down").and_then(json_to_i64) {
                outbound["down_mbps"] = json!(down);
            }
            if let Some(obfs) = node.settings.get("obfs").and_then(|v| v.as_str())
                && obfs.eq_ignore_ascii_case("salamander")
                && let Some(password) = node.settings.get("obfs-password").and_then(|v| v.as_str())
                && !password.is_empty()
            {
                outbound["obfs"] = json!({
                    "type": "salamander",
                    "password": password
                });
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

fn normalize_sing_box_server_ports(value: &str) -> Option<Vec<String>> {
    let mut normalized = Vec::new();
    for item in value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let item = if let Some((start, end)) = item.split_once('-') {
            let start = start.trim().parse::<u16>().ok()?;
            let end = end.trim().parse::<u16>().ok()?;
            if start > end {
                return None;
            }
            format!("{start}:{end}")
        } else if let Some((start, end)) = item.split_once(':') {
            let start = start.trim().parse::<u16>().ok()?;
            let end = end.trim().parse::<u16>().ok()?;
            if start > end {
                return None;
            }
            format!("{start}:{end}")
        } else {
            item.parse::<u16>().ok()?.to_string()
        };
        normalized.push(item);
    }

    (!normalized.is_empty()).then_some(normalized)
}

pub(crate) fn render_surge_proxy(node: &NodeView) -> Result<SurgeRenderResult, AppError> {
    let name = sanitize_policy_name(&node.name, node.id);
    render_surge_proxy_with_name(node, name)
}

fn render_surge_proxy_with_name(
    node: &NodeView,
    name: String,
) -> Result<SurgeRenderResult, AppError> {
    match node.protocol.as_str() {
        "shadowsocks" => {
            let method = safe_inline_value(required_json_string(&node.settings, "method")?)?;
            let password = safe_inline_value(required_json_string(&node.settings, "password")?)?;
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
            let uuid = safe_inline_value(required_json_string(&node.settings, "id")?)?;
            let mut line = format!(
                "{name} = vmess, {}, {}, username={uuid}",
                node.server, node.port
            );

            if let Some(cipher) = node.settings.get("scy").and_then(|v| v.as_str())
                && !cipher.is_empty()
            {
                let cipher = safe_inline_value(cipher)?;
                line.push_str(&format!(", encrypt-method={cipher}"));
            }
            if let Some(network) = node.settings.get("net").and_then(|v| v.as_str())
                && network.eq_ignore_ascii_case("ws")
            {
                line.push_str(", ws=true");
                if let Some(path) = node.settings.get("path").and_then(|v| v.as_str())
                    && !path.is_empty()
                {
                    let path = safe_inline_value(path)?;
                    line.push_str(&format!(", ws-path={path}"));
                }
                if let Some(host) = node.settings.get("host").and_then(|v| v.as_str())
                    && !host.is_empty()
                {
                    let host = safe_inline_value(host)?;
                    line.push_str(&format!(", ws-headers=Host:{host}"));
                }
            }
            if let Some(aead) = node.settings.get("aid").and_then(json_to_i64)
                && aead == 0
            {
                line.push_str(", vmess-aead=true");
            }
            append_common_tls_params(&mut line, &node.settings, &node.server)?;

            Ok(SurgeRenderResult {
                name,
                proxy_line: line,
                wireguard_section: None,
            })
        }
        "vless" => {
            let uuid = safe_inline_value(required_json_string(&node.settings, "uuid")?)?;
            let mut line = format!(
                "{name} = vless, {}, {}, username={uuid}",
                node.server, node.port
            );

            if let Some(flow) = node.settings.get("flow").and_then(|v| v.as_str())
                && !flow.is_empty()
            {
                let flow = safe_inline_value(flow)?;
                line.push_str(&format!(", flow={flow}"));
            }
            if let Some(network) = node.settings.get("type").and_then(|v| v.as_str())
                && network.eq_ignore_ascii_case("ws")
            {
                line.push_str(", ws=true");
                if let Some(path) = node.settings.get("path").and_then(|v| v.as_str())
                    && !path.is_empty()
                {
                    let path = safe_inline_value(path)?;
                    line.push_str(&format!(", ws-path={path}"));
                }
                if let Some(host) = node.settings.get("host").and_then(|v| v.as_str())
                    && !host.is_empty()
                {
                    let host = safe_inline_value(host)?;
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
                let fp = safe_inline_value(fp)?;
                line.push_str(&format!(", client-fingerprint={fp}"));
            }
            if let Some(pbk) = node.settings.get("pbk").and_then(|v| v.as_str())
                && !pbk.is_empty()
            {
                let pbk = safe_inline_value(pbk)?;
                line.push_str(&format!(", reality-public-key={pbk}"));
            }
            if let Some(sid) = node.settings.get("sid").and_then(|v| v.as_str())
                && !sid.is_empty()
            {
                let sid = safe_inline_value(sid)?;
                line.push_str(&format!(", reality-short-id={sid}"));
            }
            append_common_tls_params(&mut line, &node.settings, &node.server)?;

            Ok(SurgeRenderResult {
                name,
                proxy_line: line,
                wireguard_section: None,
            })
        }
        "trojan" => {
            let password = safe_inline_value(required_json_string(&node.settings, "password")?)?;
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
                    let path = safe_inline_value(path)?;
                    line.push_str(&format!(", ws-path={path}"));
                }
                if let Some(host) = node.settings.get("host").and_then(|v| v.as_str())
                    && !host.is_empty()
                {
                    let host = safe_inline_value(host)?;
                    line.push_str(&format!(", ws-headers=Host:{host}"));
                }
            }
            append_common_tls_params(&mut line, &node.settings, &node.server)?;

            Ok(SurgeRenderResult {
                name,
                proxy_line: line,
                wireguard_section: None,
            })
        }
        "hysteria2" => {
            let password = safe_inline_value(required_json_string(&node.settings, "password")?)?;
            let mut line = format!(
                "{name} = hysteria2, {}, {}, password={password}",
                node.server, node.port
            );
            if let Some(down) = node.settings.get("down").and_then(json_to_i64) {
                line.push_str(&format!(", download-bandwidth={down}"));
            }
            if let Some(obfs) = node.settings.get("obfs").and_then(|v| v.as_str())
                && obfs.eq_ignore_ascii_case("salamander")
                && let Some(obfs_password) =
                    node.settings.get("obfs-password").and_then(|v| v.as_str())
                && !obfs_password.is_empty()
            {
                let obfs_password = safe_inline_value(obfs_password)?;
                line.push_str(&format!(", salamander-password={obfs_password}"));
            }
            append_common_tls_params(&mut line, &node.settings, &node.server)?;

            Ok(SurgeRenderResult {
                name,
                proxy_line: line,
                wireguard_section: None,
            })
        }
        "tuic" => {
            let token = safe_inline_value(required_json_string(&node.settings, "password")?)?;
            let mut line = format!(
                "{name} = tuic, {}, {}, token={token}",
                node.server, node.port
            );
            if let Some(alpn) = node.settings.get("alpn").and_then(|v| v.as_str())
                && !alpn.is_empty()
            {
                let alpn = safe_inline_value(alpn)?;
                line.push_str(&format!(", alpn={alpn}"));
            }
            append_common_tls_params(&mut line, &node.settings, &node.server)?;

            Ok(SurgeRenderResult {
                name,
                proxy_line: line,
                wireguard_section: None,
            })
        }
        "wireguard" => {
            let section_name = format!("WG_{}", sanitize_section_name(&name));
            let private_key =
                safe_line_value(required_json_string(&node.settings, "private-key")?)?;
            let public_key =
                safe_inline_value(required_json_string(&node.settings, "public-key")?)?;
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
                let self_ip = safe_line_value(self_ip)?;
                section.push_str(&format!("self-ip = {self_ip}\n"));
            }
            if !self_ipv6.is_empty() {
                let self_ipv6 = safe_line_value(self_ipv6)?;
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
    render_quantumult_x_proxy_with_tag(node, &tag)
}

fn render_quantumult_x_proxy_with_tag(node: &NodeView, tag: &str) -> Result<String, AppError> {
    let endpoint = quantumult_x_endpoint(&node.server, node.port)?;
    match node.protocol.as_str() {
        "shadowsocks" => Ok(format!(
            "shadowsocks={endpoint}, method={}, password={}, tag={}",
            safe_inline_value(required_json_string(&node.settings, "method")?)?,
            safe_inline_value(required_json_string(&node.settings, "password")?)?,
            tag
        )),
        "vmess" => {
            let mut line = format!(
                "vmess={endpoint}, method=none, password={}",
                safe_inline_value(required_json_string(&node.settings, "id")?)?
            );
            append_quanx_v2ray_options(&mut line, &node.settings, &node.server, false)?;
            line.push_str(&format!(", tag={tag}"));
            Ok(line)
        }
        "vless" => {
            let mut line = format!(
                "vless={endpoint}, method=none, password={}",
                safe_inline_value(required_json_string(&node.settings, "uuid")?)?
            );
            if let Some(flow) = node.settings.get("flow").and_then(|v| v.as_str())
                && !flow.is_empty()
            {
                let flow = safe_inline_value(flow)?;
                line.push_str(&format!(", vless-flow={flow}"));
            }
            append_quanx_v2ray_options(&mut line, &node.settings, &node.server, false)?;
            line.push_str(&format!(", tag={tag}"));
            Ok(line)
        }
        "anytls" => {
            let mut line = format!(
                "anytls={endpoint}, password={}, over-tls=true",
                safe_inline_value(required_json_string(&node.settings, "password")?)?
            );
            if let Some(sni) = node.settings.get("sni").and_then(|value| value.as_str())
                && !sni.is_empty()
            {
                line.push_str(&format!(", tls-host={}", safe_inline_value(sni)?));
            }
            if node
                .settings
                .get("insecure")
                .and_then(json_to_bool)
                .unwrap_or(false)
            {
                line.push_str(", tls-verification=false");
            }
            if node
                .settings
                .get("udp")
                .and_then(json_to_bool)
                .unwrap_or(true)
            {
                line.push_str(", udp-relay=true");
            }
            if let Some(pbk) = node.settings.get("pbk").and_then(|value| value.as_str())
                && !pbk.is_empty()
            {
                line.push_str(&format!(
                    ", reality-base64-pubkey={}",
                    safe_inline_value(pbk)?
                ));
            }
            if let Some(sid) = node.settings.get("sid").and_then(|value| value.as_str())
                && !sid.is_empty()
            {
                line.push_str(&format!(
                    ", reality-hex-shortid={}",
                    safe_inline_value(sid)?
                ));
            }
            line.push_str(&format!(", tag={tag}"));
            Ok(line)
        }
        "trojan" => {
            let mut line = format!(
                "trojan={endpoint}, password={}",
                safe_inline_value(required_json_string(&node.settings, "password")?)?
            );
            append_quanx_v2ray_options(&mut line, &node.settings, &node.server, true)?;
            line.push_str(&format!(", tag={tag}"));
            Ok(line)
        }
        other => Err(AppError::BadRequest(format!(
            "node '{}' uses unsupported quanx protocol {}",
            node.name, other
        ))),
    }
}

fn quantumult_x_endpoint(server: &str, port: i64) -> Result<String, AppError> {
    let server = safe_inline_value(server)?;
    let host = if server.starts_with('[') && server.ends_with(']') {
        server
    } else if server.contains(':') {
        format!("[{server}]")
    } else {
        server
    };
    Ok(format!("{host}:{port}"))
}

fn append_quanx_v2ray_options(
    line: &mut String,
    settings: &serde_json::Value,
    server: &str,
    force_tls: bool,
) -> Result<(), AppError> {
    let network = settings
        .get("type")
        .and_then(|v| v.as_str())
        .or_else(|| settings.get("net").and_then(|v| v.as_str()));

    let uses_websocket = network.is_some_and(|network| network.eq_ignore_ascii_case("ws"));
    let uses_tls = force_tls
        || settings
            .get("security")
            .and_then(|value| value.as_str())
            .is_some_and(|security| matches!(security, "tls" | "reality"));

    if uses_websocket {
        line.push_str(if uses_tls { ", obfs=wss" } else { ", obfs=ws" });
        if let Some(path) = settings.get("path").and_then(|v| v.as_str())
            && !path.is_empty()
        {
            let path = safe_inline_value(path)?;
            line.push_str(&format!(", obfs-uri={path}"));
        }
        if let Some(host) = settings.get("host").and_then(|v| v.as_str())
            && !host.is_empty()
        {
            let host = safe_inline_value(host)?;
            line.push_str(&format!(", obfs-host={host}"));
        }
    }

    if uses_tls && !uses_websocket {
        line.push_str(", over-tls=true");
    }
    if let Some(sni) = settings.get("sni").and_then(|v| v.as_str())
        && !sni.is_empty()
        && sni != server
    {
        let sni = safe_inline_value(sni)?;
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
        let pbk = safe_inline_value(pbk)?;
        line.push_str(&format!(", reality-base64-pubkey={pbk}"));
    }
    if let Some(sid) = settings.get("sid").and_then(|v| v.as_str())
        && !sid.is_empty()
    {
        let sid = safe_inline_value(sid)?;
        line.push_str(&format!(", reality-hex-shortid={sid}"));
    }
    Ok(())
}

fn append_common_tls_params(
    line: &mut String,
    settings: &serde_json::Value,
    server: &str,
) -> Result<(), AppError> {
    if let Some(sni) = settings.get("sni").and_then(|v| v.as_str())
        && !sni.is_empty()
        && sni != server
    {
        let sni = safe_inline_value(sni)?;
        line.push_str(&format!(", sni={sni}"));
    }
    if let Some(insecure) = settings.get("insecure").and_then(json_to_bool)
        && insecure
    {
        line.push_str(", skip-cert-verify=true");
    }
    Ok(())
}

fn safe_inline_value(value: impl AsRef<str>) -> Result<String, AppError> {
    let value = value.as_ref();
    if value
        .chars()
        .any(|ch| ch == ',' || ch == '\r' || ch == '\n' || ch.is_control())
    {
        return Err(AppError::BadRequest(
            "node setting contains characters that are unsafe for line-based exports".to_string(),
        ));
    }
    Ok(value.to_string())
}

fn safe_line_value(value: impl AsRef<str>) -> Result<String, AppError> {
    let value = value.as_ref();
    if value
        .chars()
        .any(|ch| ch == '\r' || ch == '\n' || ch.is_control())
    {
        return Err(AppError::BadRequest(
            "node setting contains characters that are unsafe for line-based exports".to_string(),
        ));
    }
    Ok(value.to_string())
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
    pool: &DbPool,
    template_id: Option<i64>,
    target: &str,
) -> Result<Option<TemplateRecord>, AppError> {
    if let Some(template_id) = template_id {
        let template = template_repo::find_by_id(pool, template_id)
            .await?
            .ok_or_else(|| AppError::BadRequest(format!("template not found: {}", template_id)))?;

        if template_kind_applies(template.kind.as_str(), target) {
            return Ok(Some(template));
        }
    }

    Ok(template_repo::find_latest_builtin_by_kind(pool, target).await?)
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
        Value::Mapping(mut mapping) => {
            let builtin_kind = match template {
                Some(record)
                    if record.is_builtin != 0
                        && record.kind == "clash"
                        && record.name == BUILTIN_CLASH_TEMPLATE_NAME =>
                {
                    Some("clash")
                }
                Some(record)
                    if record.is_builtin != 0
                        && record.kind == "mihomo"
                        && record.name == BUILTIN_MIHOMO_TEMPLATE_NAME =>
                {
                    Some("mihomo")
                }
                None if target_name == "clash" => Some("clash"),
                _ => None,
            };
            if let Some(kind) = builtin_kind {
                refresh_builtin_direct_network(&mut mapping, kind)?;
            }
            Ok(mapping)
        }
        _ => Err(AppError::BadRequest(format!(
            "template '{}' must be a YAML mapping",
            template_name
        ))),
    }
}

fn built_in_clash_routing_template() -> Result<&'static str, AppError> {
    Ok(crate::services::template_seed_service::CLASH_TEMPLATE)
}

// Apply network fixes at export time so existing built-in records need no DB rewrite.
fn refresh_builtin_direct_network(root: &mut Mapping, kind: &str) -> Result<(), AppError> {
    let defaults = if kind == "mihomo" {
        crate::services::template_seed_service::MIHOMO_TEMPLATE
    } else {
        crate::services::template_seed_service::CLASH_TEMPLATE
    };
    let defaults: Value = serde_yaml::from_str(defaults).map_err(|_| AppError::Internal)?;
    root.insert(Value::String("dns".into()), defaults["dns"].clone());
    root.insert(Value::String("ipv6".into()), Value::Bool(false));
    if kind == "clash"
        && let Some(providers) = root
            .get_mut("rule-providers")
            .and_then(Value::as_mapping_mut)
    {
        for provider in providers.values_mut().filter_map(Value::as_mapping_mut) {
            if provider
                .get("url")
                .and_then(Value::as_str)
                .is_some_and(|url| url.ends_with(".list"))
            {
                provider.insert(Value::String("format".into()), Value::String("text".into()));
            }
        }
    }
    if let Some(rules) = root.get_mut("rules").and_then(Value::as_sequence_mut) {
        // Fake-IP flows may not yet have a real address when reaching the IP fallback.
        for rule in rules.iter_mut() {
            if rule.as_str() == Some("RULE-SET,cn_ip,DIRECT,no-resolve") {
                *rule = Value::String("RULE-SET,cn_ip,DIRECT".into());
            }
        }
        let fallback = "DOMAIN-SUFFIX,cn,DIRECT";
        if !rules.iter().any(|rule| rule.as_str() == Some(fallback)) {
            let index = rules
                .iter()
                .position(|rule| {
                    rule.as_str().is_some_and(|rule| {
                        rule.starts_with("RULE-SET,geolocation-not-cn,")
                            || rule.starts_with("RULE-SET,proxygfw,")
                            || rule.starts_with("MATCH,")
                    })
                })
                .unwrap_or(rules.len());
            rules.insert(index, Value::String(fallback.into()));
        }
    }
    Ok(())
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

pub(crate) fn dedupe_mihomo_proxy_names(root: &mut Mapping) {
    let proxy_key = Value::String("proxies".to_string());
    let name_key = Value::String("name".to_string());
    let mut used_names = HashSet::new();
    let mut aliases: HashMap<String, Vec<String>> = HashMap::new();

    if let Some(Value::Sequence(proxies)) = root.get_mut(&proxy_key) {
        for proxy in proxies {
            let Some(mapping) = proxy.as_mapping_mut() else {
                continue;
            };
            let Some(original_name) = mapping
                .get(&name_key)
                .and_then(Value::as_str)
                .map(str::to_string)
            else {
                continue;
            };

            let unique_name = unique_mihomo_proxy_name(&original_name, &mut used_names);
            if unique_name != original_name {
                mapping.insert(name_key.clone(), Value::String(unique_name.clone()));
            }
            aliases.entry(original_name).or_default().push(unique_name);
        }
    }

    expand_mihomo_proxy_group_refs(root, &aliases);
}

fn collect_mihomo_proxy_names(root: &Mapping) -> HashSet<String> {
    root.get(Value::String("proxies".to_string()))
        .and_then(Value::as_sequence)
        .map(|proxies| {
            proxies
                .iter()
                .filter_map(|proxy| {
                    proxy
                        .as_mapping()
                        .and_then(|mapping| mapping.get(Value::String("name".to_string())))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn unique_mihomo_proxy_name(base: &str, used_names: &mut HashSet<String>) -> String {
    let normalized = {
        let trimmed = base.trim();
        if trimmed.is_empty() { "Proxy" } else { trimmed }
    };

    if used_names.insert(normalized.to_string()) {
        return normalized.to_string();
    }

    for index in 2.. {
        let candidate = format!("{normalized} #{index}");
        if used_names.insert(candidate.clone()) {
            return candidate;
        }
    }

    unreachable!("unbounded proxy name allocator must return before usize overflow")
}

fn unique_line_proxy_name(node: &NodeView, used_names: &mut HashSet<String>) -> String {
    unique_mihomo_proxy_name(&sanitize_policy_name(&node.name, node.id), used_names)
}

fn expand_mihomo_proxy_group_refs(root: &mut Mapping, aliases: &HashMap<String, Vec<String>>) {
    if aliases.is_empty() {
        return;
    }

    let groups_key = Value::String("proxy-groups".to_string());
    let proxies_key = Value::String("proxies".to_string());
    let Some(Value::Sequence(groups)) = root.get_mut(&groups_key) else {
        return;
    };

    for group in groups {
        let Some(mapping) = group.as_mapping_mut() else {
            continue;
        };
        let Some(Value::Sequence(members)) = mapping.get_mut(&proxies_key) else {
            continue;
        };

        let mut seen = HashSet::new();
        let mut rewritten = Vec::with_capacity(members.len());
        for member in members.iter() {
            if let Some(name) = member.as_str() {
                if let Some(replacements) = aliases.get(name) {
                    for replacement in replacements {
                        if seen.insert(replacement.clone()) {
                            rewritten.push(Value::String(replacement.clone()));
                        }
                    }
                } else if seen.insert(name.to_string()) {
                    rewritten.push(Value::String(name.to_string()));
                }
            } else {
                rewritten.push(member.clone());
            }
        }

        *members = rewritten;
    }
}

fn expand_mihomo_include_all_proxy_groups(root: &mut Mapping, proxy_names: &[String]) {
    let groups_key = Value::String("proxy-groups".to_string());
    let proxies_key = Value::String("proxies".to_string());
    let include_all_key = Value::String("include-all-proxies".to_string());
    let Some(Value::Sequence(groups)) = root.get_mut(&groups_key) else {
        return;
    };

    for group in groups {
        let Some(mapping) = group.as_mapping_mut() else {
            continue;
        };
        let include_all = mapping
            .get(&include_all_key)
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !include_all {
            continue;
        }

        let members = mapping
            .entry(proxies_key.clone())
            .or_insert_with(|| Value::Sequence(Vec::new()));
        let Value::Sequence(members) = members else {
            continue;
        };

        append_unique_proxy_group_members(members, proxy_names);
        mapping.remove(&include_all_key);
    }
}

fn append_unique_proxy_group_members(members: &mut Vec<Value>, proxy_names: &[String]) {
    let mut seen = members
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<HashSet<_>>();

    for proxy_name in proxy_names {
        if seen.insert(proxy_name.clone()) {
            members.push(Value::String(proxy_name.clone()));
        }
    }
}

fn ensure_mihomo_default_proxy_groups(root: &mut Mapping, proxy_names: &[String]) {
    let groups_key = Value::String("proxy-groups".to_string());
    let has_groups = root
        .get(&groups_key)
        .and_then(Value::as_sequence)
        .is_some_and(|groups| !groups.is_empty());

    if !has_groups {
        root.insert(
            groups_key,
            Value::Sequence(vec![
                Value::Mapping(mihomo_proxy_select_group(proxy_names)),
                Value::Mapping(mihomo_manual_proxy_group(proxy_names)),
                Value::Mapping(mihomo_auto_proxy_group(proxy_names)),
                Value::Mapping(mihomo_fallback_proxy_group(proxy_names)),
                Value::Mapping(mihomo_load_balance_proxy_group(proxy_names)),
            ]),
        );
        return;
    }

    if !mihomo_proxy_group_exists(root, MIHOMO_GROUP_MANUAL)
        && !mihomo_proxy_group_exists(root, "MANUAL")
    {
        yaml_push_sequence(
            root,
            "proxy-groups",
            vec![Value::Mapping(mihomo_manual_proxy_group(proxy_names))],
        );
    }
    if !mihomo_proxy_group_exists(root, MIHOMO_GROUP_AUTO)
        && !mihomo_proxy_group_exists(root, "AUTO")
    {
        yaml_push_sequence(
            root,
            "proxy-groups",
            vec![Value::Mapping(mihomo_auto_proxy_group(proxy_names))],
        );
    }
    if !mihomo_proxy_group_exists(root, MIHOMO_GROUP_FALLBACK)
        && !mihomo_proxy_group_exists(root, "FALLBACK")
    {
        yaml_push_sequence(
            root,
            "proxy-groups",
            vec![Value::Mapping(mihomo_fallback_proxy_group(proxy_names))],
        );
    }
    if !mihomo_proxy_group_exists(root, MIHOMO_GROUP_LOAD_BALANCE)
        && !mihomo_proxy_group_exists(root, "LOAD-BALANCE")
    {
        yaml_push_sequence(
            root,
            "proxy-groups",
            vec![Value::Mapping(mihomo_load_balance_proxy_group(proxy_names))],
        );
    }
    append_groups_to_primary_selector(
        root,
        &[
            MIHOMO_GROUP_MANUAL,
            MIHOMO_GROUP_AUTO,
            MIHOMO_GROUP_FALLBACK,
            MIHOMO_GROUP_LOAD_BALANCE,
        ],
    );
}

fn ensure_mihomo_country_load_balance_groups(
    root: &mut Mapping,
    node_countries: &HashMap<i64, String>,
    proxy_names_by_node_id: &HashMap<i64, String>,
    country_load_min_nodes: usize,
    country_fallback_min_nodes: usize,
) {
    let mut proxies_by_country = HashMap::<String, Vec<String>>::new();
    for (node_id, proxy_name) in proxy_names_by_node_id {
        let Some(country_code) = node_countries.get(node_id) else {
            continue;
        };
        let country_code = country_code.trim().to_ascii_uppercase();
        if country_code.len() != 2 || !country_code.bytes().all(|byte| byte.is_ascii_alphabetic()) {
            continue;
        }
        proxies_by_country
            .entry(country_code)
            .or_default()
            .push(proxy_name.clone());
    }
    if proxies_by_country.is_empty() {
        return;
    }

    let mut load_proxies_by_group = HashMap::<&'static str, Vec<String>>::new();
    let mut fallback_proxies_by_group = HashMap::<&'static str, Vec<String>>::new();
    let mut other_region_proxies = Vec::new();
    for (country_code, mut proxy_names) in proxies_by_country {
        proxy_names.sort();
        proxy_names.dedup();
        let Some((_, load_name, fallback_name)) = MIHOMO_COUNTRY_GROUPS
            .iter()
            .find(|(code, _, _)| *code == country_code)
        else {
            other_region_proxies.extend(proxy_names);
            continue;
        };

        if proxy_names.len() < country_load_min_nodes {
            other_region_proxies.extend(proxy_names);
            continue;
        }

        load_proxies_by_group.insert(*load_name, proxy_names.clone());
        if proxy_names.len() >= country_fallback_min_nodes {
            fallback_proxies_by_group.insert(*fallback_name, proxy_names);
        }
    }
    if !other_region_proxies.is_empty() {
        other_region_proxies.sort();
        other_region_proxies.dedup();
        load_proxies_by_group.insert(MIHOMO_OTHER_COUNTRY_GROUP, other_region_proxies);
    }

    let ordered_group_names = MIHOMO_COUNTRY_GROUPS
        .iter()
        .map(|(_, load_name, fallback_name)| (*load_name, *fallback_name))
        .chain(std::iter::once((MIHOMO_OTHER_COUNTRY_GROUP, "")));
    let mut generated_group_names =
        Vec::with_capacity(load_proxies_by_group.len() + fallback_proxies_by_group.len());
    for (load_name, fallback_name) in ordered_group_names {
        let Some(proxy_names) = load_proxies_by_group.remove(load_name) else {
            continue;
        };
        if !mihomo_proxy_group_exists(root, load_name) {
            yaml_push_sequence(
                root,
                "proxy-groups",
                vec![Value::Mapping(mihomo_country_load_balance_group(
                    load_name,
                    &proxy_names,
                ))],
            );
        }
        generated_group_names.push(load_name);
        let Some(fallback_proxy_names) = fallback_proxies_by_group.remove(fallback_name) else {
            continue;
        };
        if !mihomo_proxy_group_exists(root, fallback_name) {
            yaml_push_sequence(
                root,
                "proxy-groups",
                vec![Value::Mapping(mihomo_scoped_fallback_group(
                    fallback_name,
                    &fallback_proxy_names,
                ))],
            );
        }
        generated_group_names.push(fallback_name);
    }
    append_groups_to_primary_selector(root, &generated_group_names);
}

fn ensure_mihomo_fraud_groups(
    root: &mut Mapping,
    node_countries: &HashMap<i64, String>,
    node_risks: &HashMap<i64, NodeIpProbeRecord>,
    proxy_names_by_node_id: &HashMap<i64, String>,
) {
    remove_reserved_mihomo_fraud_groups(root);

    let mut groups = collect_risk_policy_groups(node_countries, node_risks, proxy_names_by_node_id);

    let mut generated_names = Vec::new();
    push_reserved_fraud_group(
        root,
        MIHOMO_GROUP_ZERO_FRAUD.to_string(),
        &mut groups.zero_all,
        &mut generated_names,
    );
    push_reserved_fraud_group(
        root,
        MIHOMO_GROUP_LOW_FRAUD.to_string(),
        &mut groups.low_all,
        &mut generated_names,
    );
    push_reserved_select_group(
        root,
        MIHOMO_GROUP_RESIDENTIAL_MANUAL.to_string(),
        &mut groups.residential_all,
        &mut generated_names,
    );
    for (country_code, mut proxies) in groups.zero_by_country {
        push_reserved_fraud_group(
            root,
            format!("{}故障转移0欺诈", mihomo_country_label(&country_code)),
            &mut proxies,
            &mut generated_names,
        );
    }
    for (country_code, mut proxies) in groups.low_by_country {
        push_reserved_fraud_group(
            root,
            format!("{}故障转移低欺诈", mihomo_country_label(&country_code)),
            &mut proxies,
            &mut generated_names,
        );
    }
    let generated_refs = generated_names
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    append_groups_to_primary_selector(root, &generated_refs);
}

#[derive(Default)]
struct RiskPolicyGroups {
    zero_all: Vec<String>,
    low_all: Vec<String>,
    residential_all: Vec<String>,
    zero_by_country: BTreeMap<String, Vec<String>>,
    low_by_country: BTreeMap<String, Vec<String>>,
}

fn collect_risk_policy_groups(
    node_countries: &HashMap<i64, String>,
    node_risks: &HashMap<i64, NodeIpProbeRecord>,
    proxy_names_by_node_id: &HashMap<i64, String>,
) -> RiskPolicyGroups {
    let mut groups = RiskPolicyGroups::default();
    let now_unix_ms = OffsetDateTime::now_utc().unix_timestamp() * 1_000;

    for (node_id, proxy_name) in proxy_names_by_node_id {
        let Some(risk) = node_risks.get(node_id) else {
            continue;
        };
        if risk.qualifies_for_residential(now_unix_ms) {
            groups.residential_all.push(proxy_name.clone());
        }
        let target = if risk.qualifies_for_zero_fraud(now_unix_ms) {
            groups.zero_all.push(proxy_name.clone());
            &mut groups.zero_by_country
        } else if risk.qualifies_for_low_fraud(now_unix_ms) {
            groups.low_all.push(proxy_name.clone());
            &mut groups.low_by_country
        } else {
            continue;
        };
        let Some(country_code) = node_countries.get(node_id) else {
            continue;
        };
        let country_code = country_code.trim().to_ascii_uppercase();
        if country_code.len() == 2 && country_code.bytes().all(|byte| byte.is_ascii_alphabetic()) {
            target
                .entry(country_code)
                .or_default()
                .push(proxy_name.clone());
        }
    }

    sort_and_dedupe_policy_members(&mut groups.zero_all);
    sort_and_dedupe_policy_members(&mut groups.low_all);
    sort_and_dedupe_policy_members(&mut groups.residential_all);
    for members in groups.zero_by_country.values_mut() {
        sort_and_dedupe_policy_members(members);
    }
    for members in groups.low_by_country.values_mut() {
        sort_and_dedupe_policy_members(members);
    }
    groups
}

fn sort_and_dedupe_policy_members(members: &mut Vec<String>) {
    members.sort();
    members.dedup();
}

fn push_reserved_select_group(
    root: &mut Mapping,
    name: String,
    proxies: &mut Vec<String>,
    generated_names: &mut Vec<String>,
) {
    proxies.sort();
    proxies.dedup();
    if proxies.is_empty() {
        return;
    }
    yaml_push_sequence(
        root,
        "proxy-groups",
        vec![Value::Mapping(mihomo_select_proxy_group(&name, proxies))],
    );
    generated_names.push(name);
}

fn push_reserved_fraud_group(
    root: &mut Mapping,
    name: String,
    proxies: &mut Vec<String>,
    generated_names: &mut Vec<String>,
) {
    proxies.sort();
    proxies.dedup();
    if proxies.is_empty() {
        return;
    }
    yaml_push_sequence(
        root,
        "proxy-groups",
        vec![Value::Mapping(mihomo_scoped_fallback_group(&name, proxies))],
    );
    generated_names.push(name);
}

fn mihomo_country_label(country_code: &str) -> String {
    let country_code = country_code.trim().to_ascii_uppercase();
    COUNTRY_NAMES_ZH
        .iter()
        .find(|(code, _)| *code == country_code)
        .map(|(_, name)| *name)
        .unwrap_or("未知地区")
        .to_string()
}

fn remove_reserved_mihomo_fraud_groups(root: &mut Mapping) {
    let Some(Value::Sequence(groups)) = root.get_mut(Value::String("proxy-groups".to_string()))
    else {
        return;
    };
    let reserved = |name: &str| {
        name == MIHOMO_GROUP_ZERO_FRAUD
            || name == MIHOMO_GROUP_LOW_FRAUD
            || name == MIHOMO_GROUP_RESIDENTIAL_MANUAL
            || name.ends_with("故障转移0欺诈")
            || name.ends_with("故障转移低欺诈")
    };
    let removed = groups
        .iter()
        .filter_map(|group| {
            group
                .as_mapping()
                .and_then(|mapping| mapping.get(Value::String("name".to_string())))
                .and_then(Value::as_str)
                .filter(|name| reserved(name))
                .map(str::to_string)
        })
        .collect::<HashSet<_>>();
    groups.retain(|group| {
        !group
            .as_mapping()
            .and_then(|mapping| mapping.get(Value::String("name".to_string())))
            .and_then(Value::as_str)
            .is_some_and(&reserved)
    });
    if removed.is_empty() {
        return;
    }
    for group in groups.iter_mut().filter_map(Value::as_mapping_mut) {
        if let Some(Value::Sequence(proxies)) = group.get_mut(Value::String("proxies".to_string()))
        {
            proxies.retain(|proxy| proxy.as_str().is_none_or(|name| !removed.contains(name)));
        }
    }
}

fn mihomo_scoped_fallback_group(name: &str, proxy_names: &[String]) -> Mapping {
    let mut group = mihomo_health_check_proxy_group(name, "fallback", proxy_names, None);
    group.insert(
        Value::String("interval".to_string()),
        Value::Number(120.into()),
    );
    group.insert(
        Value::String("timeout".to_string()),
        Value::Number(5000.into()),
    );
    group.insert(
        Value::String("max-failed-times".to_string()),
        Value::Number(2.into()),
    );
    group.insert(
        Value::String("expected-status".to_string()),
        Value::String("204".to_string()),
    );
    group
}

fn mihomo_country_load_balance_group(name: &str, proxy_names: &[String]) -> Mapping {
    let mut group = Mapping::new();
    group.insert(
        Value::String("name".to_string()),
        Value::String(name.to_string()),
    );
    group.insert(
        Value::String("type".to_string()),
        Value::String("load-balance".to_string()),
    );
    group.insert(
        Value::String("strategy".to_string()),
        Value::String("consistent-hashing".to_string()),
    );
    group.insert(
        Value::String("proxies".to_string()),
        Value::Sequence(proxy_names.iter().cloned().map(Value::String).collect()),
    );
    group.insert(
        Value::String("url".to_string()),
        Value::String(MIHOMO_DEFAULT_DELAY_TEST_URL.to_string()),
    );
    group.insert(
        Value::String("interval".to_string()),
        Value::Number(300.into()),
    );
    group.insert(Value::String("lazy".to_string()), Value::Bool(true));
    group
}

fn append_groups_to_primary_selector(root: &mut Mapping, group_names: &[&str]) {
    let Some(Value::Sequence(groups)) = root.get_mut(Value::String("proxy-groups".to_string()))
    else {
        return;
    };
    let selector_index = groups
        .iter()
        .position(|group| {
            group.as_mapping().is_some_and(|mapping| {
                mapping
                    .get(Value::String("name".to_string()))
                    .and_then(Value::as_str)
                    == Some(MIHOMO_GROUP_PROXY)
            })
        })
        .or_else(|| {
            groups.iter().position(|group| {
                group.as_mapping().is_some_and(|mapping| {
                    mapping
                        .get(Value::String("type".to_string()))
                        .and_then(Value::as_str)
                        == Some("select")
                })
            })
        });
    let Some(mapping) = selector_index
        .and_then(|index| groups.get_mut(index))
        .and_then(Value::as_mapping_mut)
    else {
        return;
    };
    let members = mapping
        .entry(Value::String("proxies".to_string()))
        .or_insert_with(|| Value::Sequence(Vec::new()));
    let Some(members) = members.as_sequence_mut() else {
        return;
    };
    let mut seen = members
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<HashSet<_>>();
    let direct_index = members
        .iter()
        .position(|member| member.as_str() == Some("DIRECT"))
        .unwrap_or(members.len());
    let mut insert_at = direct_index;
    for group_name in group_names {
        if seen.insert((*group_name).to_string()) {
            members.insert(insert_at, Value::String((*group_name).to_string()));
            insert_at += 1;
        }
    }
}

fn mihomo_proxy_group_exists(root: &Mapping, name: &str) -> bool {
    root.get(Value::String("proxy-groups".to_string()))
        .and_then(Value::as_sequence)
        .is_some_and(|groups| {
            groups.iter().any(|group| {
                group
                    .as_mapping()
                    .and_then(|mapping| mapping.get(Value::String("name".to_string())))
                    .and_then(Value::as_str)
                    .is_some_and(|group_name| group_name == name)
            })
        })
}

fn mihomo_first_proxy_group_name(root: &Mapping) -> Option<String> {
    root.get(Value::String("proxy-groups".to_string()))
        .and_then(Value::as_sequence)
        .and_then(|groups| {
            groups.iter().find_map(|group| {
                group
                    .as_mapping()
                    .and_then(|mapping| mapping.get(Value::String("name".to_string())))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
        })
}

fn ensure_mihomo_match_rule(root: &mut Mapping) {
    let rules_key = Value::String("rules".to_string());
    let has_rules = root
        .get(&rules_key)
        .and_then(Value::as_sequence)
        .is_some_and(|rules| !rules.is_empty());
    if has_rules {
        return;
    }

    let target_group = if mihomo_proxy_group_exists(root, MIHOMO_GROUP_PROXY) {
        MIHOMO_GROUP_PROXY.to_string()
    } else if mihomo_proxy_group_exists(root, MIHOMO_GROUP_AUTO) {
        MIHOMO_GROUP_AUTO.to_string()
    } else if let Some(group_name) = mihomo_first_proxy_group_name(root) {
        group_name
    } else {
        "DIRECT".to_string()
    };

    root.insert(
        rules_key,
        Value::Sequence(vec![Value::String(format!("MATCH,{target_group}"))]),
    );
}

fn compact_builtin_mihomo_policy_groups(root: &mut Mapping, template: Option<&TemplateRecord>) {
    let Some(template) = template.filter(|template| template.is_builtin != 0) else {
        return;
    };
    let (aliases, removed_groups) = match (template.kind.as_str(), template.name.as_str()) {
        ("clash", BUILTIN_CLASH_TEMPLATE_NAME) => (
            BUILTIN_CLASH_COMPACT_GROUP_ALIASES,
            BUILTIN_CLASH_REMOVED_GROUPS,
        ),
        ("mihomo", BUILTIN_MIHOMO_TEMPLATE_NAME) => (
            BUILTIN_MIHOMO_COMPACT_GROUP_ALIASES,
            BUILTIN_MIHOMO_REMOVED_GROUPS,
        ),
        _ => return,
    };

    if let Some(groups) = root
        .get_mut(Value::String("proxy-groups".to_string()))
        .and_then(Value::as_sequence_mut)
    {
        groups.retain(|group| {
            let Some(name) = group
                .as_mapping()
                .and_then(|group| group.get(Value::String("name".to_string())))
                .and_then(Value::as_str)
            else {
                return true;
            };
            !removed_groups.contains(&name)
        });
    }

    rewrite_mihomo_group_references(root, aliases);
}

fn serialize_mihomo_yaml(
    root: &Mapping,
    template: Option<&TemplateRecord>,
) -> Result<String, AppError> {
    let yaml = serde_yaml::to_string(root).map_err(|_| AppError::Internal)?;
    let is_builtin_mihomo = template.is_some_and(|template| {
        template.is_builtin != 0
            && template.kind == "mihomo"
            && template.name == BUILTIN_MIHOMO_TEMPLATE_NAME
    });
    if !is_builtin_mihomo {
        return Ok(yaml);
    }

    let providers = MIHOMO_INLINE_RULE_PROVIDERS_BLOCK.get_or_init(|| {
        let mut block = String::with_capacity(MIHOMO_INLINE_RULE_PROVIDERS_YAML.len() + 1024);
        block.push_str("rule-providers:\n");
        for line in MIHOMO_INLINE_RULE_PROVIDERS_YAML.lines() {
            block.push_str("  ");
            block.push_str(line);
            block.push('\n');
        }
        block
    });
    replace_top_level_yaml_section(&yaml, "rule-providers", providers).ok_or_else(|| {
        tracing::error!("built-in Mihomo template is missing its rule-providers section");
        AppError::Internal
    })
}

fn replace_top_level_yaml_section(
    yaml: &str,
    section_name: &str,
    replacement: &str,
) -> Option<String> {
    let marker = format!("{section_name}:\n");
    let start = if yaml.starts_with(&marker) {
        0
    } else {
        yaml.find(&format!("\n{marker}"))? + 1
    };
    let content_start = start + marker.len();
    let mut end = yaml.len();
    let mut cursor = content_start;
    for line in yaml[content_start..].split_inclusive('\n') {
        let line_without_newline = line.strip_suffix('\n').unwrap_or(line);
        if !line_without_newline.is_empty() && !line_without_newline.starts_with([' ', '\t', '#']) {
            end = cursor;
            break;
        }
        cursor += line.len();
    }

    let mut result = String::with_capacity(yaml.len() - (end - start) + replacement.len());
    result.push_str(&yaml[..start]);
    result.push_str(replacement);
    result.push_str(&yaml[end..]);
    Some(result)
}

fn localize_mihomo_legacy_group_names(root: &mut Mapping) {
    let existing_group_names = root
        .get(Value::String("proxy-groups".to_string()))
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(Value::as_mapping)
        .filter_map(|group| group.get(Value::String("name".to_string())))
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<HashSet<_>>();
    let aliases = MIHOMO_LEGACY_GROUP_NAMES
        .iter()
        .copied()
        .filter(|(legacy, localized)| {
            existing_group_names.contains(*legacy) && !existing_group_names.contains(*localized)
        })
        .collect::<Vec<_>>();
    if aliases.is_empty() {
        return;
    }

    rewrite_mihomo_group_references(root, &aliases);
}

fn rewrite_mihomo_group_references(root: &mut Mapping, aliases: &[(&str, &str)]) {
    if let Some(groups) = root
        .get_mut(Value::String("proxy-groups".to_string()))
        .and_then(Value::as_sequence_mut)
    {
        for group in groups {
            let Some(group) = group.as_mapping_mut() else {
                continue;
            };
            if let Some(name) = group
                .get_mut(Value::String("name".to_string()))
                .and_then(|value| value.as_str())
                .and_then(|name| localized_mihomo_group_name(name, aliases))
            {
                group.insert(
                    Value::String("name".to_string()),
                    Value::String(name.to_string()),
                );
            }
            if let Some(members) = group
                .get_mut(Value::String("proxies".to_string()))
                .and_then(Value::as_sequence_mut)
            {
                for member in members {
                    let Some(localized) = member
                        .as_str()
                        .and_then(|name| localized_mihomo_group_name(name, aliases))
                        .map(str::to_string)
                    else {
                        continue;
                    };
                    *member = Value::String(localized);
                }
            }
        }
    }

    if let Some(rules) = root
        .get_mut(Value::String("rules".to_string()))
        .and_then(Value::as_sequence_mut)
    {
        for rule in rules {
            let Some(rule_text) = rule.as_str() else {
                continue;
            };
            for (legacy, localized) in aliases {
                let suffix = format!(",{legacy}");
                let no_resolve_suffix = format!(",{legacy},no-resolve");
                if let Some(prefix) = rule_text.strip_suffix(&no_resolve_suffix) {
                    *rule = Value::String(format!("{prefix},{localized},no-resolve"));
                    break;
                }
                if let Some(prefix) = rule_text.strip_suffix(&suffix) {
                    *rule = Value::String(format!("{prefix},{localized}"));
                    break;
                }
            }
        }
    }

    if let Some(dns) = root.get_mut(Value::String("dns".to_string())) {
        localize_mihomo_dns_group_references(dns, aliases);
    }
}

fn localized_mihomo_group_name<'a>(name: &str, aliases: &[(&'a str, &'a str)]) -> Option<&'a str> {
    aliases
        .iter()
        .find_map(|(legacy, localized)| (*legacy == name).then_some(*localized))
}

fn localize_mihomo_dns_group_references(value: &mut Value, aliases: &[(&str, &str)]) {
    match value {
        Value::String(text) => {
            for (legacy, localized) in aliases {
                *text = text.replace(&format!("#{legacy}"), &format!("#{localized}"));
            }
        }
        Value::Sequence(items) => {
            for item in items {
                localize_mihomo_dns_group_references(item, aliases);
            }
        }
        Value::Mapping(mapping) => {
            for item in mapping.values_mut() {
                localize_mihomo_dns_group_references(item, aliases);
            }
        }
        _ => {}
    }
}

fn mihomo_proxy_select_group(proxy_names: &[String]) -> Mapping {
    let mut members = vec![
        Value::String(MIHOMO_GROUP_MANUAL.to_string()),
        Value::String(MIHOMO_GROUP_AUTO.to_string()),
        Value::String(MIHOMO_GROUP_FALLBACK.to_string()),
        Value::String(MIHOMO_GROUP_LOAD_BALANCE.to_string()),
        Value::String("DIRECT".to_string()),
    ];
    if proxy_names.is_empty() {
        members.retain(|member| member.as_str() != Some(MIHOMO_GROUP_MANUAL));
    }

    let mut group = Mapping::new();
    group.insert(
        Value::String("name".to_string()),
        Value::String(MIHOMO_GROUP_PROXY.to_string()),
    );
    group.insert(
        Value::String("type".to_string()),
        Value::String("select".to_string()),
    );
    group.insert(
        Value::String("proxies".to_string()),
        Value::Sequence(members),
    );
    group
}

fn mihomo_manual_proxy_group(proxy_names: &[String]) -> Mapping {
    mihomo_select_proxy_group(MIHOMO_GROUP_MANUAL, proxy_names)
}

fn mihomo_select_proxy_group(name: &str, proxy_names: &[String]) -> Mapping {
    let mut group = Mapping::new();
    group.insert(
        Value::String("name".to_string()),
        Value::String(name.to_string()),
    );
    group.insert(
        Value::String("type".to_string()),
        Value::String("select".to_string()),
    );
    group.insert(
        Value::String("proxies".to_string()),
        Value::Sequence(proxy_names.iter().cloned().map(Value::String).collect()),
    );
    group
}

fn mihomo_auto_proxy_group(proxy_names: &[String]) -> Mapping {
    let mut group = Mapping::new();
    group.insert(
        Value::String("name".to_string()),
        Value::String(MIHOMO_GROUP_AUTO.to_string()),
    );
    group.insert(
        Value::String("type".to_string()),
        Value::String("url-test".to_string()),
    );
    group.insert(
        Value::String("proxies".to_string()),
        Value::Sequence(proxy_names.iter().cloned().map(Value::String).collect()),
    );
    group.insert(
        Value::String("url".to_string()),
        Value::String(MIHOMO_DEFAULT_DELAY_TEST_URL.to_string()),
    );
    group.insert(
        Value::String("interval".to_string()),
        Value::Number(300.into()),
    );
    group
}

fn mihomo_fallback_proxy_group(proxy_names: &[String]) -> Mapping {
    mihomo_health_check_proxy_group(MIHOMO_GROUP_FALLBACK, "fallback", proxy_names, None)
}

fn mihomo_load_balance_proxy_group(proxy_names: &[String]) -> Mapping {
    mihomo_health_check_proxy_group(
        MIHOMO_GROUP_LOAD_BALANCE,
        "load-balance",
        proxy_names,
        Some("consistent-hashing"),
    )
}

fn mihomo_health_check_proxy_group(
    name: &str,
    group_type: &str,
    proxy_names: &[String],
    strategy: Option<&str>,
) -> Mapping {
    let mut group = Mapping::new();
    group.insert(
        Value::String("name".to_string()),
        Value::String(name.to_string()),
    );
    group.insert(
        Value::String("type".to_string()),
        Value::String(group_type.to_string()),
    );
    if let Some(strategy) = strategy {
        group.insert(
            Value::String("strategy".to_string()),
            Value::String(strategy.to_string()),
        );
    }
    group.insert(
        Value::String("proxies".to_string()),
        Value::Sequence(proxy_names.iter().cloned().map(Value::String).collect()),
    );
    group.insert(
        Value::String("url".to_string()),
        Value::String(MIHOMO_DEFAULT_DELAY_TEST_URL.to_string()),
    );
    group.insert(
        Value::String("interval".to_string()),
        Value::Number(300.into()),
    );
    group.insert(Value::String("lazy".to_string()), Value::Bool(true));
    group
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
    if sections
        .iter()
        .any(|section| section.name.eq_ignore_ascii_case(name))
    {
        return;
    }

    sections.push(SurgeSection {
        name: name.to_string(),
        lines,
    });
}

fn append_lines_to_section(sections: &mut Vec<SurgeSection>, name: &str, mut lines: Vec<String>) {
    if let Some(section) = sections
        .iter_mut()
        .find(|section| section.name.eq_ignore_ascii_case(name))
    {
        section.lines.append(&mut lines);
        return;
    }

    sections.push(SurgeSection {
        name: name.to_string(),
        lines,
    });
}

#[derive(Clone, Copy)]
enum IniPolicyFormat {
    QuantumultX,
    Shadowrocket,
}

fn configure_ini_risk_policy_groups(
    sections: &mut Vec<SurgeSection>,
    format: IniPolicyFormat,
    groups: &RiskPolicyGroups,
) {
    let section_name = match format {
        IniPolicyFormat::QuantumultX => "policy",
        IniPolicyFormat::Shadowrocket => "Proxy Group",
    };
    ensure_section(sections, section_name, Vec::new());
    let section = sections
        .iter_mut()
        .find(|section| section.name.eq_ignore_ascii_case(section_name))
        .expect("policy section must exist after ensure_section");

    section.lines.retain(|line| {
        ini_policy_name(line, format).is_none_or(|name| !is_reserved_risk_policy_name(name))
    });

    let (generated_names, generated_lines) = render_ini_risk_policy_groups(format, groups);
    rewrite_ini_primary_selector(&mut section.lines, format, &generated_names);
    section.lines.extend(generated_lines);
}

fn configure_shadowrocket_base_policy_groups(
    sections: &mut Vec<SurgeSection>,
    proxy_names: &[String],
) {
    ensure_section(sections, "Proxy Group", Vec::new());
    let section = sections
        .iter_mut()
        .find(|section| section.name.eq_ignore_ascii_case("Proxy Group"))
        .expect("proxy group section must exist after ensure_section");

    section.lines.retain(|line| {
        ini_policy_name(line, IniPolicyFormat::Shadowrocket).is_none_or(|name| {
            !matches!(
                name,
                MIHOMO_GROUP_PROXY | MIHOMO_GROUP_MANUAL | MIHOMO_GROUP_AUTO
            )
        })
    });

    let members = proxy_names.join(", ");
    section.lines.splice(
        0..0,
        [
            format!(
                "{MIHOMO_GROUP_PROXY} = select, {MIHOMO_GROUP_MANUAL}, {MIHOMO_GROUP_AUTO}, direct"
            ),
            format!("{MIHOMO_GROUP_MANUAL} = select, {members}"),
            format!(
                "{MIHOMO_GROUP_AUTO} = url-test, {members}, url={MIHOMO_DEFAULT_DELAY_TEST_URL}, interval=600, tolerance=0"
            ),
        ],
    );
}

fn render_ini_risk_policy_groups(
    format: IniPolicyFormat,
    groups: &RiskPolicyGroups,
) -> (Vec<String>, Vec<String>) {
    let mut names = Vec::new();
    let mut lines = Vec::new();
    push_ini_risk_policy(
        format,
        MIHOMO_GROUP_ZERO_FRAUD,
        &groups.zero_all,
        false,
        &mut names,
        &mut lines,
    );
    push_ini_risk_policy(
        format,
        MIHOMO_GROUP_LOW_FRAUD,
        &groups.low_all,
        false,
        &mut names,
        &mut lines,
    );
    push_ini_risk_policy(
        format,
        MIHOMO_GROUP_RESIDENTIAL_MANUAL,
        &groups.residential_all,
        true,
        &mut names,
        &mut lines,
    );
    for (country_code, members) in &groups.zero_by_country {
        push_ini_risk_policy(
            format,
            &format!("{}故障转移0欺诈", mihomo_country_label(country_code)),
            members,
            false,
            &mut names,
            &mut lines,
        );
    }
    for (country_code, members) in &groups.low_by_country {
        push_ini_risk_policy(
            format,
            &format!("{}故障转移低欺诈", mihomo_country_label(country_code)),
            members,
            false,
            &mut names,
            &mut lines,
        );
    }
    (names, lines)
}

fn push_ini_risk_policy(
    format: IniPolicyFormat,
    name: &str,
    members: &[String],
    manual: bool,
    names: &mut Vec<String>,
    lines: &mut Vec<String>,
) {
    if members.is_empty() {
        return;
    }
    let members = members.join(", ");
    let line = match (format, manual) {
        (IniPolicyFormat::QuantumultX, true) => format!("static={name}, {members}"),
        (IniPolicyFormat::QuantumultX, false) => format!(
            "available={name}, {members}, check-interval=300, tolerance=0, alive-checking=false"
        ),
        (IniPolicyFormat::Shadowrocket, true) => format!("{name} = select, {members}"),
        (IniPolicyFormat::Shadowrocket, false) => format!(
            "{name} = fallback, {members}, url={MIHOMO_DEFAULT_DELAY_TEST_URL}, interval=300"
        ),
    };
    names.push(name.to_string());
    lines.push(line);
}

fn ini_policy_name(line: &str, format: IniPolicyFormat) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
        return None;
    }
    let (left, right) = trimmed.split_once('=')?;
    match format {
        IniPolicyFormat::QuantumultX => right.split(',').next().map(str::trim),
        IniPolicyFormat::Shadowrocket => Some(left.trim()),
    }
}

fn is_reserved_risk_policy_name(name: &str) -> bool {
    name == MIHOMO_GROUP_ZERO_FRAUD
        || name == MIHOMO_GROUP_LOW_FRAUD
        || name == MIHOMO_GROUP_RESIDENTIAL_MANUAL
        || name.ends_with("故障转移0欺诈")
        || name.ends_with("故障转移低欺诈")
}

fn rewrite_ini_primary_selector(
    lines: &mut Vec<String>,
    format: IniPolicyFormat,
    generated_names: &[String],
) {
    let primary_name = match format {
        IniPolicyFormat::QuantumultX => QUANX_GROUP_PROXY,
        IniPolicyFormat::Shadowrocket => MIHOMO_GROUP_PROXY,
    };
    let primary_index = lines
        .iter()
        .position(|line| ini_policy_name(line, format).is_some_and(|name| name == primary_name));

    let Some(index) = primary_index else {
        let generated = generated_names.join(", ");
        let line = match format {
            IniPolicyFormat::QuantumultX if generated.is_empty() => {
                format!("static={primary_name}, 自动选择, direct")
            }
            IniPolicyFormat::QuantumultX => {
                format!("static={primary_name}, 自动选择, {generated}, direct")
            }
            IniPolicyFormat::Shadowrocket if generated.is_empty() => {
                format!("{primary_name} = select, 自动选择, direct")
            }
            IniPolicyFormat::Shadowrocket => {
                format!("{primary_name} = select, 自动选择, {generated}, direct")
            }
        };
        lines.insert(0, line);
        return;
    };

    let Some((left, right)) = lines[index].split_once('=') else {
        return;
    };
    let mut tokens = right
        .split(',')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    tokens.retain(|token| !is_reserved_risk_policy_name(token));

    let first_member = 1.min(tokens.len());
    let insert_at = tokens[first_member..]
        .iter()
        .position(|token| {
            token.eq_ignore_ascii_case("direct")
                || token.eq_ignore_ascii_case("reject")
                || token.contains('=')
        })
        .map(|index| first_member + index)
        .unwrap_or(tokens.len());
    for (offset, name) in generated_names.iter().enumerate() {
        if !tokens.iter().any(|token| token == name) {
            tokens.insert(insert_at + offset, name.clone());
        }
    }
    lines[index] = match format {
        IniPolicyFormat::QuantumultX => {
            format!("{}={}", left.trim(), tokens.join(", "))
        }
        IniPolicyFormat::Shadowrocket => {
            format!("{} = {}", left.trim(), tokens.join(", "))
        }
    };
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
        } else if network.eq_ignore_ascii_case("grpc")
            && let Some(service_name) = settings.get("grpc-service-name").and_then(|v| v.as_str())
            && !service_name.is_empty()
        {
            outbound["transport"]["service_name"] = json!(service_name);
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

fn insert_serialized_json_mapping(
    map: &mut Mapping,
    output_key: &str,
    settings: &serde_json::Value,
    settings_key: &str,
) -> bool {
    let Some(raw) = settings.get(settings_key).and_then(|value| value.as_str()) else {
        return false;
    };
    let Ok(json_value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return false;
    };
    let Ok(Value::Mapping(mapping)) = serde_yaml::to_value(json_value) else {
        return false;
    };
    map.insert(
        Value::String(output_key.to_string()),
        Value::Mapping(mapping),
    );
    true
}

fn split_csv_scalars(input: &str) -> Vec<Value> {
    input
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .parse::<i64>()
                .map(|number| Value::Number(number.into()))
                .unwrap_or_else(|_| Value::String(value.to_string()))
        })
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
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate, max-age=0"),
    );
    response
        .headers_mut()
        .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    response.headers_mut().insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("x-robots-tag"),
        HeaderValue::from_static("noindex, nofollow, noarchive"),
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
    format!("attachment; filename=\"{ascii_fallback}\"; filename*=UTF-8''{encoded}")
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

#[cfg(test)]
mod tests {
    #[test]
    fn nodes_only_exports_drop_rules_without_losing_node_settings() {
        let yaml = "dns: {enable: true}\nrules: [MATCH,DIRECT]\nproxy-groups: []\nproxies:\n  - {name: test, type: ss, server: example.com, password: test}\n";
        let value: serde_yaml::Value =
            serde_yaml::from_str(&super::nodes_only_export(yaml, "mihomo").unwrap()).unwrap();
        assert_eq!(value.as_mapping().unwrap().len(), 4);
        assert_eq!(value["rules"][0], "MATCH,DIRECT");
        assert_eq!(value["mode"], "rule");
        assert!(value.get("proxy-groups").is_some());
        assert_eq!(value["proxies"][0]["password"], "test");
        let json = r#"{"dns":{},"route":{},"outbounds":[{"type":"selector","tag":"proxy"},{"type":"direct"},{"type":"trojan","tag":"test","password":"kept"}]}"#;
        let value: serde_json::Value =
            serde_json::from_str(&super::nodes_only_export(json, "sing-box").unwrap()).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 1);
        assert_eq!(value["outbounds"].as_array().unwrap().len(), 3);
        assert_eq!(value["outbounds"][2]["password"], "kept");
        for target in ["surge", "shadowrocket-profile", "loon", "surfboard", "quan"] {
            let output = super::nodes_only_export("[General]\ndns-server=1.1.1.1\n[Proxy]\nnode=ss,example.com,443\n[WireGuard test]\nprivate-key=test\n[Rule]\nFINAL,proxy\n", target).unwrap();
            assert!(output.contains("node=ss"));
            assert!(output.contains("private-key=test"));
            assert!(!output.contains("dns-server"));
            assert!(!output.contains("FINAL"));
        }
        let output = super::nodes_only_export(
            "[general]\nx=1\n[server_local]\nshadowsocks=test\n[policy]\nstatic=proxy\n",
            "quanx",
        )
        .unwrap();
        assert_eq!(
            output,
            "[server_local]\nshadowsocks=test\n[policy]\nstatic=proxy\n"
        );
        assert_eq!(
            super::nodes_only_export("ss://example", "uri-bundle").unwrap(),
            "ss://example"
        );
    }

    #[test]
    fn nodes_only_yaml_replaces_website_rules_with_selected_group_match() {
        for (groups, expected) in [
            ("[{name: 自动选择}, {name: 节点选择}]", "节点选择"),
            ("[{name: Custom}]", "Custom"),
            ("[{name: 自动选择}]", "自动选择"),
        ] {
            let source = format!(
                "mode: direct\ndns: {{enable: true}}\nrule-providers: {{ads: {{type: http}}}}\nproxies: []\nproxy-providers: {{upstream: {{type: file}}}}\nproxy-groups: {groups}\nrules:\n  - DOMAIN-SUFFIX,example.com,DIRECT\n  - RULE-SET,ads,REJECT\n  - MATCH,DIRECT\n"
            );
            for target in ["mihomo", "mellow"] {
                let output = super::nodes_only_export(&source, target).unwrap();
                let value: Value = serde_yaml::from_str(&output).unwrap();
                assert_eq!(value["mode"], "rule");
                assert_eq!(
                    value["rules"],
                    serde_yaml::to_value(vec![format!("MATCH,{expected}")]).unwrap()
                );
                assert!(value.get("dns").is_none());
                assert!(value.get("rule-providers").is_none());
                assert!(value.get("proxy-providers").is_some());
                assert_eq!(super::nodes_only_export(&output, target).unwrap(), output);
            }
        }
    }

    #[test]
    fn content_choice_has_a_distinct_cache_key() {
        assert_ne!(
            super::public_export_cache_key(
                1,
                "token",
                "same",
                None,
                "mihomo",
                super::ExportMode::Strict,
                true
            ),
            super::public_export_cache_key(
                1,
                "token",
                "same",
                None,
                "mihomo",
                super::ExportMode::Strict,
                false
            )
        );
    }

    #[tokio::test]
    async fn nodes_only_keeps_generated_country_load_and_fallback_groups() {
        let mut subscription = sample_shadowsocks_subscription("mihomo");
        let original = subscription.nodes[0].clone();
        for id in [2, 3] {
            let mut node = original.clone();
            node.id = id;
            node.name = format!("JP-{id}");
            subscription.nodes.push(node);
        }
        let countries = subscription
            .nodes
            .iter()
            .map(|node| (node.id, "JP".to_string()))
            .collect();
        let risks = HashMap::new();
        let response = export_mihomo(
            subscription,
            ExportMode::Strict,
            None,
            "mihomo",
            &ExportNodeMetadata {
                countries: &countries,
                risks: &risks,
            },
            2,
            3,
        )
        .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let full: Value = serde_yaml::from_slice(&body).unwrap();
        let nodes: Value = serde_yaml::from_str(
            &super::nodes_only_export(std::str::from_utf8(&body).unwrap(), "mihomo").unwrap(),
        )
        .unwrap();
        assert_eq!(full["proxy-groups"], nodes["proxy-groups"]);
        let groups = nodes["proxy-groups"].as_sequence().unwrap();
        assert!(groups.iter().any(|group| group["type"] == "load-balance"));
        assert!(groups.iter().any(|group| group["type"] == "fallback"));
        assert!(nodes.get("dns").is_none());
        assert_eq!(
            nodes["rules"],
            serde_yaml::to_value(vec!["MATCH,代理选择"]).unwrap()
        );
        assert!(nodes.get("rule-providers").is_none());
    }

    use axum::http::header;

    use super::{
        BUILTIN_CLASH_TEMPLATE_NAME, BUILTIN_MIHOMO_TEMPLATE_NAME, BUILTIN_QUANX_TEMPLATE_NAME,
        ExportMode, ExportNodeMetadata, MIHOMO_DEFAULT_DELAY_TEST_URL,
        MIHOMO_INLINE_RULE_PROVIDERS_YAML, PUBLIC_EXPORT_CACHE_BODY_LIMIT, canonical_export_target,
        compact_builtin_mihomo_policy_groups, dedupe_mihomo_proxy_names,
        ensure_mihomo_country_load_balance_groups, ensure_mihomo_default_proxy_groups,
        ensure_mihomo_fraud_groups, ensure_mihomo_match_rule,
        expand_mihomo_include_all_proxy_groups, export_legacy_client_profile, export_mihomo,
        export_quantumult_x, export_shadowrocket_profile, export_sing_box, export_ssd,
        export_sssub, is_xray_supported, load_export_template, localize_mihomo_legacy_group_names,
        mihomo_country_label, mihomo_proxy_group_exists, normalize_sing_box_server_ports,
        render_quantumult_x_proxy, resolve_export_mode, safe_inline_value, safe_line_value,
        text_response, unique_mihomo_proxy_name,
    };
    use crate::domain::{
        node::NodeView,
        node_ip_probe::{NodeIpProbeRecord, NodeRiskTraits},
        subscription::SubscriptionView,
        template::TemplateRecord,
    };
    use serde_yaml::{Mapping, Value};
    use std::{
        collections::{HashMap, HashSet},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn sample_shadowsocks_subscription(default_client: &str) -> SubscriptionView {
        SubscriptionView {
            id: 1,
            name: "Template test".to_string(),
            token: "test-token".to_string(),
            description: String::new(),
            default_client: Some(default_client.to_string()),
            template_id: None,
            group_id: None,
            enabled: true,
            expires_at: None,
            status: "active".to_string(),
            include_rules: true,
            portal_enabled: false,
            portal_slug: None,
            portal_access_code_set: false,
            node_group_ids: Vec::new(),
            node_ids: vec![1],
            nodes: vec![NodeView {
                id: 1,
                name: "SS node".to_string(),
                protocol: "shadowsocks".to_string(),
                raw_link: String::new(),
                server: "192.0.2.1".to_string(),
                port: 443,
                enabled: true,
                group_id: None,
                source_type: "manual".to_string(),
                source_ref: None,
                upstream_missing: false,
                fingerprint: "fingerprint-1".to_string(),
                settings: serde_json::json!({
                    "method": "aes-128-gcm",
                    "password": "test-password"
                }),
                remark: String::new(),
                last_latency_ms: None,
                last_latency_status: None,
                last_latency_message: None,
                last_latency_tested_at: None,
                created_at: String::new(),
                updated_at: String::new(),
            }],
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn sample_template(kind: &str, content: &str) -> TemplateRecord {
        TemplateRecord {
            id: 1,
            name: format!("Test {kind} template"),
            kind: kind.to_string(),
            content: content.to_string(),
            is_builtin: 1,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn risk_record(node_id: i64, status: &str, score: i64) -> NodeIpProbeRecord {
        let traits = NodeRiskTraits {
            usage_type: Some("residential".to_string()),
            is_proxy: Some(false),
            is_vpn: Some(false),
            is_tor: Some(false),
            is_hosting: Some(false),
            is_abuser: Some(false),
            is_relay: Some(false),
            threat_level: None,
            is_botnet_c2: None,
            conflicts: Vec::new(),
        };
        NodeIpProbeRecord {
            node_id,
            status: "ok".to_string(),
            ip: Some("1.1.1.1".to_string()),
            ip_version: Some(4),
            exit_ip_revision: 1,
            country_code: None,
            country_name: None,
            country_source: None,
            intelligence_status: Some("enriched".to_string()),
            intelligence_message: None,
            risk_ip: Some("1.1.1.1".to_string()),
            risk_status: Some(status.to_string()),
            scamalytics_fraud_score: Some(score),
            scamalytics_isp_risk_score: Some(score),
            risk_checked_at: Some("2026-09-02T00:00:00Z".to_string()),
            risk_expires_at_unix_ms: Some(1_893_456_000_000),
            risk_message: None,
            risk_traits_json: Some(serde_json::to_string(&traits).unwrap()),
            risk_traits_expires_at_unix_ms: Some(1_893_456_000_000),
            message: None,
            probed_at: "2026-09-02T00:00:00Z".to_string(),
            country_updated_at: None,
            intelligence_updated_at: None,
            updated_at: "2026-09-02T00:00:00Z".to_string(),
        }
    }

    async fn response_body(response: axum::response::Response) -> String {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should be readable");
        String::from_utf8(body.to_vec()).expect("response body should be UTF-8")
    }

    #[tokio::test]
    async fn selects_system_templates_by_target_unless_an_explicit_template_applies() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let database_relative_path = format!(
            "target/test-data/sublinkx-export-template-{}-{nonce}.db",
            std::process::id()
        );
        let database_path = std::env::current_dir()
            .expect("current directory should exist")
            .join(&database_relative_path);
        let pool = crate::db::new_database_pool(&format!("sqlite://{database_relative_path}"))
            .await
            .expect("test database should initialize");
        crate::services::template_seed_service::seed_default_templates(&pool)
            .await
            .expect("system templates should seed");

        for kind in [
            "xray",
            "clash",
            "mihomo",
            "surge",
            "sing-box",
            "surge3",
            "surge2",
            "quanx",
            "shadowrocket-profile",
            "quan",
            "loon",
            "surfboard",
            "mellow",
            "clashr",
            "ss",
            "sssub",
            "ssr",
            "ssd",
            "trojan",
            "mixed",
        ] {
            let template = load_export_template(&pool, None, kind)
                .await
                .expect("system template lookup should succeed")
                .unwrap_or_else(|| panic!("system template should exist for {kind}"));
            assert_eq!(template.kind, kind);
            assert_eq!(template.is_builtin, 1);
        }

        let mihomo = load_export_template(&pool, None, "mihomo")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(mihomo.name, "Built-in Mihomo Policy Orchestrator");

        let now = "2026-09-01T00:00:00Z";
        let explicit = crate::repository::template_repo::insert(
            &pool,
            &crate::repository::template_repo::NewTemplateRecord {
                name: "Explicit Mihomo template",
                kind: "mihomo",
                content: "rules: [MATCH,DIRECT]",
                is_builtin: 0,
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .expect("explicit template should insert");
        assert_eq!(
            load_export_template(&pool, Some(explicit.id), "mihomo")
                .await
                .unwrap()
                .unwrap()
                .id,
            explicit.id
        );
        assert_eq!(
            load_export_template(&pool, Some(explicit.id), "sing-box")
                .await
                .unwrap()
                .unwrap()
                .name,
            "Built-in sing-box Route Base"
        );

        pool.close().await;
        for attempt in 0..10 {
            match std::fs::remove_file(&database_path) {
                Ok(()) => break,
                Err(error) if attempt < 9 => {
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                    if error.kind() == std::io::ErrorKind::NotFound {
                        break;
                    }
                }
                Err(error) => panic!("test database should be removable: {error}"),
            }
        }
    }

    #[tokio::test]
    async fn template_backed_exporters_preserve_configuration_and_add_nodes() {
        let sssub_template =
            sample_template("sssub", r#"{"version":1,"custom":"kept","servers":[]}"#);
        let sssub = response_body(
            export_sssub(
                sample_shadowsocks_subscription("sssub"),
                ExportMode::Strict,
                Some(&sssub_template),
            )
            .unwrap(),
        )
        .await;
        let sssub = serde_json::from_str::<serde_json::Value>(&sssub).unwrap();
        assert_eq!(sssub["custom"], "kept");
        assert_eq!(sssub["servers"].as_array().unwrap().len(), 1);

        let ssd_template =
            sample_template("ssd", r#"{"airport":"old","custom":"kept","servers":[]}"#);
        let ssd = response_body(
            export_ssd(
                sample_shadowsocks_subscription("ssd"),
                ExportMode::Strict,
                Some(&ssd_template),
            )
            .unwrap(),
        )
        .await;
        let ssd = serde_json::from_str::<serde_json::Value>(&ssd).unwrap();
        assert_eq!(ssd["airport"], "Template test");
        assert_eq!(ssd["custom"], "kept");
        assert_eq!(ssd["servers"].as_array().unwrap().len(), 1);

        let quanx_template = sample_template(
            "quanx",
            "[general]\ncustom = kept\n\n[server_local]\n\n[policy]\nstatic=节点选择, 自动选择, direct\n\n[filter_remote]\n",
        );
        let quanx = response_body(
            export_quantumult_x(
                sample_shadowsocks_subscription("quanx"),
                ExportMode::Strict,
                Some(&quanx_template),
                &ExportNodeMetadata {
                    countries: &HashMap::new(),
                    risks: &HashMap::new(),
                },
            )
            .unwrap(),
        )
        .await;
        assert!(quanx.contains("custom = kept"));
        assert!(quanx.contains("192.0.2.1"));
        assert!(quanx.contains("[server_local]"));

        for (kind, client_name, template_content, preserved_line) in [
            (
                "quan",
                "Quantumult",
                "[SERVER]\n\n[POLICY]\nstatic=PROXY, auto, direct\n",
                "static=PROXY, auto, direct",
            ),
            (
                "loon",
                "Loon",
                "[General]\ncustom = kept\n\n[Proxy]\n\n[Rule]\nFINAL,Proxy\n",
                "custom = kept",
            ),
            (
                "surfboard",
                "Surfboard",
                "[General]\ncustom = kept\n\n[Proxy]\n\n[Rule]\nFINAL,Proxy\n",
                "FINAL,Proxy",
            ),
        ] {
            let template = sample_template(kind, template_content);
            let body = response_body(
                export_legacy_client_profile(
                    sample_shadowsocks_subscription(kind),
                    ExportMode::Strict,
                    client_name,
                    Some(&template),
                )
                .unwrap(),
            )
            .await;
            assert!(
                body.contains(preserved_line),
                "{kind} should retain template content"
            );
            assert!(
                body.contains("192.0.2.1"),
                "{kind} should contain the selected node"
            );
        }

        let mut mihomo_template = sample_template(
            "mihomo",
            r#"dns:
  enable: true
  nameserver: [1.1.1.1]
rule-providers:
  custom:
    type: http
    behavior: domain
    url: https://example.invalid/rules.yaml
    path: ./rules/custom.yaml
rules:
  - RULE-SET,custom,代理选择
"#,
        );
        mihomo_template.name = "Built-in Mihomo Policy Orchestrator".to_string();
        mihomo_template.is_builtin = 0;
        let mihomo = response_body(
            export_mihomo(
                sample_shadowsocks_subscription("mihomo"),
                ExportMode::Strict,
                Some(&mihomo_template),
                "mihomo",
                &ExportNodeMetadata {
                    countries: &HashMap::new(),
                    risks: &HashMap::new(),
                },
                2,
                3,
            )
            .unwrap(),
        )
        .await;
        assert!(mihomo.contains("dns:"));
        assert!(mihomo.contains("rule-providers:"));
        assert!(mihomo.contains("type: http"));
        assert!(mihomo.contains("https://example.invalid/rules.yaml"));
        assert!(mihomo.contains("192.0.2.1"));
    }

    #[tokio::test]
    async fn quanx_and_shadowrocket_profiles_share_risk_policy_groups() {
        let mut subscription = sample_shadowsocks_subscription("quanx");
        subscription.nodes[0].name = "JP Zero".to_string();
        let mut low_node = subscription.nodes[0].clone();
        low_node.id = 2;
        low_node.name = "US Low".to_string();
        low_node.server = "198.51.100.2".to_string();
        subscription.nodes.push(low_node);
        subscription.node_ids = vec![1, 2];

        let countries = HashMap::from([(1, "JP".to_string()), (2, "US".to_string())]);
        let risks = HashMap::from([
            (1, risk_record(1, "zero", 0)),
            (2, risk_record(2, "low", 12)),
        ]);
        let metadata = ExportNodeMetadata {
            countries: &countries,
            risks: &risks,
        };

        let quanx = response_body(
            export_quantumult_x(subscription.clone(), ExportMode::Strict, None, &metadata).unwrap(),
        )
        .await;
        assert!(quanx.contains("[server_local]"));
        assert!(!quanx.contains("[server_remote]"));
        assert!(quanx.contains("available=0欺诈, JP Zero"));
        assert!(quanx.contains("available=低欺诈, US Low"));
        assert!(quanx.contains("static=手动选择住宅网络, JP Zero"));
        assert!(quanx.contains("available=日本故障转移0欺诈, JP Zero"));
        assert!(quanx.contains("available=美国故障转移低欺诈, US Low"));
        assert!(quanx.contains("static=节点选择, 自动选择, 0欺诈, 低欺诈, 手动选择住宅网络"));

        let shadowrocket = response_body(
            export_shadowrocket_profile(subscription, ExportMode::Strict, None, &metadata).unwrap(),
        )
        .await;
        assert!(shadowrocket.contains("[Proxy]"));
        assert!(shadowrocket.contains("手动选择 = select, JP Zero, US Low"));
        assert!(shadowrocket.contains(
            "自动选择 = url-test, JP Zero, US Low, url=https://cp.cloudflare.com/generate_204"
        ));
        assert!(shadowrocket.contains("0欺诈 = fallback, JP Zero"));
        assert!(shadowrocket.contains("低欺诈 = fallback, US Low"));
        assert!(shadowrocket.contains("手动选择住宅网络 = select, JP Zero"));
        assert!(shadowrocket.contains("日本故障转移0欺诈 = fallback, JP Zero"));
        assert!(shadowrocket.contains("美国故障转移低欺诈 = fallback, US Low"));
        assert!(
            shadowrocket
                .contains("代理选择 = select, 手动选择, 自动选择, 0欺诈, 低欺诈, 手动选择住宅网络")
        );
        assert!(shadowrocket.contains("FINAL,代理选择"));
        assert!(!shadowrocket.contains("FINAL,PROXY"));
    }

    #[tokio::test]
    async fn quanx_complete_profile_filters_unsupported_hysteria2_nodes() {
        let mut subscription = sample_shadowsocks_subscription("quanx");
        let mut hysteria2 = subscription.nodes[0].clone();
        hysteria2.id = 2;
        hysteria2.name = "HY2 unsupported by QX".to_string();
        hysteria2.protocol = "hysteria2".to_string();
        hysteria2.settings = serde_json::json!({
            "password": "secret",
            "sni": "hy.example.com"
        });
        subscription.nodes.push(hysteria2);
        subscription.node_ids.push(2);

        let response = export_quantumult_x(
            subscription,
            ExportMode::BestEffort,
            None,
            &ExportNodeMetadata {
                countries: &HashMap::new(),
                risks: &HashMap::new(),
            },
        )
        .unwrap();

        assert_eq!(
            response
                .headers()
                .get("x-sublinkx-filtered-count")
                .and_then(|value| value.to_str().ok()),
            Some("1")
        );
        let body = response_body(response).await;
        assert!(body.contains("shadowsocks=192.0.2.1:443"));
        assert!(!body.contains("hysteria2="));
        assert!(!body.contains("HY2 unsupported by QX"));
    }

    #[tokio::test]
    async fn quanx_rebuild_removes_stale_reserved_policy_groups() {
        let template = sample_template(
            "quanx",
            "[server_local]\n\n[policy]\nstatic=节点选择, 日本故障转移0欺诈, direct\navailable=日本故障转移0欺诈, STALE, check-interval=300\n",
        );
        let body = response_body(
            export_quantumult_x(
                sample_shadowsocks_subscription("quanx"),
                ExportMode::Strict,
                Some(&template),
                &ExportNodeMetadata {
                    countries: &HashMap::new(),
                    risks: &HashMap::new(),
                },
            )
            .unwrap(),
        )
        .await;

        assert!(!body.contains("日本故障转移0欺诈"));
        assert!(!body.contains("STALE"));
        assert!(body.contains("static=节点选择, direct"));
    }

    #[tokio::test]
    async fn quanx_builtin_export_ignores_stale_persisted_template_content() {
        let mut template = sample_template(
            "quanx",
            "[general]\nserver_check_url = https://www.gstatic.com/generate_204\n\n[server_remote]\n",
        );
        template.name = BUILTIN_QUANX_TEMPLATE_NAME.to_string();

        let body = response_body(
            export_quantumult_x(
                sample_shadowsocks_subscription("quanx"),
                ExportMode::Strict,
                Some(&template),
                &ExportNodeMetadata {
                    countries: &HashMap::new(),
                    risks: &HashMap::new(),
                },
            )
            .unwrap(),
        )
        .await;

        assert!(
            body.starts_with("[general]\nserver_check_url=http://cp.cloudflare.com/generate_204\n")
        );
        assert!(body.contains("[server_local]"));
        assert!(!body.contains("[server_remote]"));
        assert!(!body.contains("geo_location_checker"));
    }

    #[test]
    fn keeps_legacy_shadowrocket_target_separate_from_full_profile() {
        assert_eq!(canonical_export_target("shadowrocket").unwrap(), "xray");
        assert_eq!(
            canonical_export_target("shadowrocket-profile").unwrap(),
            "shadowrocket-profile"
        );
        assert_eq!(
            canonical_export_target("shadowrocket-config").unwrap(),
            "shadowrocket-profile"
        );
    }

    #[test]
    fn renders_quantumult_x_vless_reality_vision_with_official_syntax() {
        let mut node = sample_shadowsocks_subscription("quanx").nodes.remove(0);
        node.protocol = "vless".to_string();
        node.name = "AU 01".to_string();
        node.server = "direct-au.example.com".to_string();
        node.port = 36699;
        node.settings = serde_json::json!({
            "uuid": "4c374a1d-e334-4ec1-b010-489bfa360ba9",
            "flow": "xtls-rprx-vision",
            "type": "tcp",
            "security": "reality",
            "sni": "swdist.apple.com",
            "pbk": "base64-key",
            "sid": "5f7b"
        });

        let rendered = render_quantumult_x_proxy(&node).unwrap();
        assert!(rendered.starts_with(
            "vless=direct-au.example.com:36699, method=none, password=4c374a1d-e334-4ec1-b010-489bfa360ba9"
        ));
        assert!(rendered.contains(", vless-flow=xtls-rprx-vision"));
        assert!(!rendered.contains(", flow="));
        assert!(rendered.contains(", over-tls=true"));
        assert!(rendered.contains(", reality-base64-pubkey=base64-key"));
        assert!(rendered.contains(", reality-hex-shortid=5f7b"));
        assert!(rendered.ends_with(", tag=AU 01"));
    }

    #[test]
    fn renders_quantumult_x_ipv6_endpoint_with_brackets() {
        let mut node = sample_shadowsocks_subscription("quanx").nodes.remove(0);
        node.server = "2001:db8::1".to_string();

        let rendered = render_quantumult_x_proxy(&node).unwrap();
        assert!(rendered.starts_with("shadowsocks=[2001:db8::1]:443, "));
        assert!(rendered.ends_with("tag=SS node"));
    }

    #[test]
    fn renders_quantumult_x_anytls_with_tls_and_reality_fields() {
        let mut node = sample_shadowsocks_subscription("quanx").nodes.remove(0);
        node.protocol = "anytls".to_string();
        node.name = "AnyTLS node".to_string();
        node.settings = serde_json::json!({
            "password": "secret",
            "sni": "edge.example.com",
            "insecure": true,
            "udp": true,
            "pbk": "base64-key",
            "sid": "0123456789abcdef"
        });

        let rendered = render_quantumult_x_proxy(&node).unwrap();
        assert!(rendered.starts_with("anytls=192.0.2.1:443"));
        assert!(rendered.contains("password=secret"));
        assert!(rendered.contains("over-tls=true"));
        assert!(rendered.contains("tls-host=edge.example.com"));
        assert!(rendered.contains("tls-verification=false"));
        assert!(rendered.contains("udp-relay=true"));
        assert!(rendered.contains("reality-base64-pubkey=base64-key"));
        assert!(rendered.contains("reality-hex-shortid=0123456789abcdef"));
        assert!(rendered.ends_with("tag=AnyTLS node"));
    }

    #[tokio::test]
    async fn built_in_mihomo_export_embeds_rule_providers() {
        let mut template = sample_template(
            "mihomo",
            crate::services::template_seed_service::MIHOMO_TEMPLATE,
        );
        template.name = "Built-in Mihomo Policy Orchestrator".to_string();

        let body = response_body(
            export_mihomo(
                sample_shadowsocks_subscription("mihomo"),
                ExportMode::Strict,
                Some(&template),
                "mihomo",
                &ExportNodeMetadata {
                    countries: &HashMap::new(),
                    risks: &HashMap::new(),
                },
                2,
                3,
            )
            .unwrap(),
        )
        .await;
        assert!(MIHOMO_INLINE_RULE_PROVIDERS_YAML.len() < 512 * 1024);
        assert!(body.len() < 1024 * 1024);
        assert!(body.len() < PUBLIC_EXPORT_CACHE_BODY_LIMIT);
        assert!(!body.contains("raw.githubusercontent.com/MetaCubeX/meta-rules-dat"));

        let root = serde_yaml::from_str::<Value>(&body).unwrap();
        let providers = root
            .as_mapping()
            .and_then(|root| root.get(Value::String("rule-providers".to_string())))
            .and_then(Value::as_mapping)
            .unwrap();
        assert_eq!(providers.len(), 23);
        assert_eq!(
            providers
                .values()
                .filter_map(Value::as_mapping)
                .filter_map(|provider| provider.get(Value::String("payload".to_string())))
                .filter_map(Value::as_sequence)
                .map(Vec::len)
                .sum::<usize>(),
            17_937
        );

        for provider in providers.values() {
            let provider = provider.as_mapping().unwrap();
            assert_eq!(
                provider
                    .get(Value::String("type".to_string()))
                    .and_then(Value::as_str),
                Some("inline")
            );
            assert!(
                provider
                    .get(Value::String("payload".to_string()))
                    .and_then(Value::as_sequence)
                    .is_some_and(|payload| !payload.is_empty())
            );
            for remote_only_key in ["url", "path", "interval", "format"] {
                assert!(!provider.contains_key(Value::String(remote_only_key.to_string())));
            }
        }

        let provider_names = providers
            .keys()
            .filter_map(Value::as_str)
            .collect::<HashSet<_>>();
        let rules = root
            .as_mapping()
            .and_then(|root| root.get(Value::String("rules".to_string())))
            .and_then(Value::as_sequence)
            .unwrap();
        for provider_name in rules.iter().filter_map(Value::as_str).filter_map(|rule| {
            rule.strip_prefix("RULE-SET,")
                .and_then(|rule| rule.split(',').next())
        }) {
            assert!(
                provider_names.contains(provider_name),
                "rule references missing provider {provider_name}"
            );
        }

        let nameserver_policy = root
            .as_mapping()
            .and_then(|root| root.get(Value::String("dns".to_string())))
            .and_then(Value::as_mapping)
            .and_then(|dns| dns.get(Value::String("nameserver-policy".to_string())))
            .and_then(Value::as_mapping)
            .unwrap();
        for provider_name in nameserver_policy
            .keys()
            .filter_map(Value::as_str)
            .filter_map(|key| key.strip_prefix("rule-set:"))
        {
            assert!(
                provider_names.contains(provider_name),
                "DNS policy references missing provider {provider_name}"
            );
        }

        let cn_domain_count = providers
            .get(Value::String("cn_domain".to_string()))
            .and_then(Value::as_mapping)
            .and_then(|provider| provider.get(Value::String("payload".to_string())))
            .and_then(Value::as_sequence)
            .map(Vec::len);
        assert_eq!(cn_domain_count, Some(3_691));
    }

    #[test]
    fn rejects_inline_export_delimiters() {
        assert!(safe_inline_value("secret, udp-relay=true").is_err());
        assert!(safe_inline_value("secret\nProxy = direct").is_err());
    }

    #[test]
    fn rejects_line_breaks_in_section_values() {
        assert!(safe_line_value("private-key\npeer = injected").is_err());
    }

    #[test]
    fn normalizes_mihomo_hysteria2_port_ranges_for_sing_box() {
        assert_eq!(
            normalize_sing_box_server_ports("20000-30000, 443, 40000:41000"),
            Some(vec![
                "20000:30000".to_string(),
                "443".to_string(),
                "40000:41000".to_string()
            ])
        );
        assert_eq!(normalize_sing_box_server_ports("30000-20000"), None);
        assert_eq!(normalize_sing_box_server_ports("not-a-port"), None);
    }

    #[test]
    fn adaptive_client_requests_always_use_best_effort_mode() {
        assert_eq!(
            resolve_export_mode(None, Some("strict"), Some("xray")).unwrap(),
            ExportMode::BestEffort
        );
        assert_eq!(
            resolve_export_mode(Some("xray"), Some("strict"), Some("xray")).unwrap(),
            ExportMode::Strict
        );
        assert_eq!(
            resolve_export_mode(None, Some("strict"), None).unwrap(),
            ExportMode::Strict
        );
        assert!(resolve_export_mode(None, Some("invalid"), Some("xray")).is_err());
    }

    #[test]
    fn v2rayn_uri_bundle_accepts_sing_box_backed_share_protocols() {
        for protocol in ["tuic", "wireguard", "anytls"] {
            let node = NodeView {
                id: 1,
                name: format!("{protocol} node"),
                protocol: protocol.to_string(),
                raw_link: format!("{protocol}://example"),
                server: "192.0.2.1".to_string(),
                port: 443,
                enabled: true,
                group_id: None,
                source_type: "manual".to_string(),
                source_ref: None,
                upstream_missing: false,
                fingerprint: format!("fingerprint-{protocol}"),
                settings: serde_json::json!({}),
                remark: String::new(),
                last_latency_ms: None,
                last_latency_status: None,
                last_latency_message: None,
                last_latency_tested_at: None,
                created_at: String::new(),
                updated_at: String::new(),
            };
            assert!(is_xray_supported(&node), "{protocol} should be exported");
        }
    }

    #[test]
    fn export_response_disables_cache_and_referrers() {
        let response = text_response(
            "proxy config".to_string(),
            "text/plain; charset=utf-8",
            "subscription.txt",
            ExportMode::Strict,
            0,
        )
        .unwrap();

        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store, no-cache, must-revalidate, max-age=0")
        );
        assert_eq!(
            response
                .headers()
                .get("referrer-policy")
                .and_then(|value| value.to_str().ok()),
            Some("no-referrer")
        );
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_DISPOSITION)
                .and_then(|value| value.to_str().ok()),
            Some("attachment; filename=\"subscription.txt\"; filename*=UTF-8''subscription%2Etxt")
        );
        assert_eq!(
            response
                .headers()
                .get("x-robots-tag")
                .and_then(|value| value.to_str().ok()),
            Some("noindex, nofollow, noarchive")
        );
    }

    #[test]
    fn dedupes_mihomo_proxy_names_and_expands_group_refs() {
        let value = serde_yaml::from_str::<Value>(
            r#"
proxies:
  - name: JP 日本01[HY2]
    type: hysteria2
    server: one.example
    port: 443
  - name: JP 日本01[HY2]
    type: hysteria2
    server: two.example
    port: 443
proxy-groups:
  - name: AUTO
    type: select
    proxies:
      - JP 日本01[HY2]
"#,
        )
        .unwrap();
        let mut root = value.as_mapping().unwrap().clone();

        dedupe_mihomo_proxy_names(&mut root);

        let proxies = root
            .get(Value::String("proxies".to_string()))
            .and_then(Value::as_sequence)
            .unwrap();
        let names = proxies
            .iter()
            .map(|proxy| {
                proxy
                    .as_mapping()
                    .unwrap()
                    .get(Value::String("name".to_string()))
                    .and_then(Value::as_str)
                    .unwrap()
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["JP 日本01[HY2]", "JP 日本01[HY2] #2"]);

        let group_members = root
            .get(Value::String("proxy-groups".to_string()))
            .and_then(Value::as_sequence)
            .and_then(|groups| groups.first())
            .and_then(Value::as_mapping)
            .and_then(|group| group.get(Value::String("proxies".to_string())))
            .and_then(Value::as_sequence)
            .unwrap()
            .iter()
            .map(|member| member.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(group_members, vec!["JP 日本01[HY2]", "JP 日本01[HY2] #2"]);
    }

    #[test]
    fn allocates_next_available_mihomo_proxy_name() {
        let mut used = HashSet::from(["Proxy".to_string(), "Proxy #2".to_string()]);

        assert_eq!(unique_mihomo_proxy_name("Proxy", &mut used), "Proxy #3");
        assert_eq!(unique_mihomo_proxy_name("", &mut used), "Proxy #4");
    }

    #[test]
    fn expands_include_all_proxy_groups_to_explicit_members() {
        let value = serde_yaml::from_str::<Value>(
            r#"
proxy-groups:
  - name: PROXY
    type: select
    include-all-proxies: true
    proxies:
      - AUTO
      - DIRECT
  - name: AUTO
    type: url-test
    include-all-proxies: true
"#,
        )
        .unwrap();
        let mut root = value.as_mapping().unwrap().clone();
        let proxy_names = vec![
            "JP 日本01[HY2]".to_string(),
            "JP 日本02[HY2]".to_string(),
            "JP 日本03[HY2]".to_string(),
        ];

        expand_mihomo_include_all_proxy_groups(&mut root, &proxy_names);

        assert!(mihomo_proxy_group_exists(&root, "AUTO"));
        let groups = root
            .get(Value::String("proxy-groups".to_string()))
            .and_then(Value::as_sequence)
            .unwrap();
        let proxy_group = groups
            .first()
            .and_then(Value::as_mapping)
            .expect("PROXY group should be present");
        assert!(
            proxy_group
                .get(Value::String("include-all-proxies".to_string()))
                .is_none()
        );
        let members = proxy_group
            .get(Value::String("proxies".to_string()))
            .and_then(Value::as_sequence)
            .unwrap()
            .iter()
            .map(|member| member.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            members,
            vec![
                "AUTO",
                "DIRECT",
                "JP 日本01[HY2]",
                "JP 日本02[HY2]",
                "JP 日本03[HY2]"
            ]
        );
    }

    #[test]
    fn creates_url_test_auto_group_for_plain_mihomo_exports() {
        let mut root = serde_yaml::from_str::<Value>("{}\n")
            .unwrap()
            .as_mapping()
            .unwrap()
            .clone();
        let proxy_names = vec!["JP 日本01[HY2]".to_string(), "JP 日本02[HY2]".to_string()];

        ensure_mihomo_default_proxy_groups(&mut root, &proxy_names);
        ensure_mihomo_match_rule(&mut root);

        let groups = root
            .get(Value::String("proxy-groups".to_string()))
            .and_then(Value::as_sequence)
            .unwrap();
        let proxy_group = groups.first().and_then(Value::as_mapping).unwrap();
        assert_eq!(
            proxy_group
                .get(Value::String("name".to_string()))
                .and_then(Value::as_str),
            Some("代理选择")
        );
        assert_eq!(
            proxy_group
                .get(Value::String("type".to_string()))
                .and_then(Value::as_str),
            Some("select")
        );

        let manual_group = groups.get(1).and_then(Value::as_mapping).unwrap();
        assert_eq!(
            manual_group
                .get(Value::String("name".to_string()))
                .and_then(Value::as_str),
            Some("手动选择")
        );
        assert_eq!(
            manual_group
                .get(Value::String("type".to_string()))
                .and_then(Value::as_str),
            Some("select")
        );

        let auto_group = groups.get(2).and_then(Value::as_mapping).unwrap();
        assert_eq!(
            auto_group
                .get(Value::String("name".to_string()))
                .and_then(Value::as_str),
            Some("自动选择")
        );
        assert_eq!(
            auto_group
                .get(Value::String("type".to_string()))
                .and_then(Value::as_str),
            Some("url-test")
        );
        assert_eq!(
            auto_group
                .get(Value::String("url".to_string()))
                .and_then(Value::as_str),
            Some(MIHOMO_DEFAULT_DELAY_TEST_URL)
        );
        let fallback_group = groups.get(3).and_then(Value::as_mapping).unwrap();
        assert_eq!(
            fallback_group
                .get(Value::String("name".to_string()))
                .and_then(Value::as_str),
            Some("故障转移")
        );
        assert_eq!(
            fallback_group
                .get(Value::String("type".to_string()))
                .and_then(Value::as_str),
            Some("fallback")
        );
        let load_balance_group = groups.get(4).and_then(Value::as_mapping).unwrap();
        assert_eq!(
            load_balance_group
                .get(Value::String("type".to_string()))
                .and_then(Value::as_str),
            Some("load-balance")
        );
        assert_eq!(
            load_balance_group
                .get(Value::String("strategy".to_string()))
                .and_then(Value::as_str),
            Some("consistent-hashing")
        );
        let proxy_members = proxy_group
            .get(Value::String("proxies".to_string()))
            .and_then(Value::as_sequence)
            .unwrap()
            .iter()
            .map(|member| member.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            proxy_members,
            vec!["手动选择", "自动选择", "故障转移", "负载均衡", "DIRECT"]
        );

        let rules = root
            .get(Value::String("rules".to_string()))
            .and_then(Value::as_sequence)
            .unwrap();
        assert_eq!(
            rules.first().and_then(Value::as_str),
            Some("MATCH,代理选择")
        );
    }

    #[test]
    fn localizes_legacy_mihomo_group_names_and_references() {
        let value = serde_yaml::from_str::<Value>(
            r#"
dns:
  nameserver:
    - https://1.1.1.1/dns-query#PROXY
proxy-groups:
  - name: PROXY
    type: select
    proxies: [MANUAL, AUTO, DIRECT]
  - name: MANUAL
    type: select
    proxies: [Node]
  - name: AUTO
    type: url-test
    proxies: [Node]
rules:
  - DOMAIN-SUFFIX,example.com,PROXY
  - IP-CIDR,198.51.100.0/24,AUTO,no-resolve
"#,
        )
        .unwrap();
        let mut root = value.as_mapping().unwrap().clone();

        localize_mihomo_legacy_group_names(&mut root);
        let yaml = serde_yaml::to_string(&root).unwrap();

        assert!(yaml.contains("name: 代理选择"));
        assert!(yaml.contains("name: 手动选择"));
        assert!(yaml.contains("name: 自动选择"));
        assert!(yaml.contains("DOMAIN-SUFFIX,example.com,代理选择"));
        assert!(yaml.contains("IP-CIDR,198.51.100.0/24,自动选择,no-resolve"));
        assert!(yaml.contains("dns-query#代理选择"));
        assert!(!yaml.contains("name: PROXY"));
    }

    #[tokio::test]
    async fn does_not_export_database_node_groups_as_proxy_groups() {
        let nodes = [1_i64, 2_i64]
            .into_iter()
            .map(|id| NodeView {
                id,
                name: format!("Grouped-{id}"),
                protocol: "shadowsocks".to_string(),
                raw_link: String::new(),
                server: format!("192.0.2.{id}"),
                port: 443,
                enabled: true,
                group_id: Some(10),
                source_type: "manual".to_string(),
                source_ref: None,
                upstream_missing: false,
                fingerprint: format!("fingerprint-{id}"),
                settings: serde_json::json!({
                    "method": "aes-128-gcm",
                    "password": "test-password"
                }),
                remark: String::new(),
                last_latency_ms: None,
                last_latency_status: None,
                last_latency_message: None,
                last_latency_tested_at: None,
                created_at: String::new(),
                updated_at: String::new(),
            })
            .collect::<Vec<_>>();
        let subscription = SubscriptionView {
            id: 1,
            name: "Grouped subscription".to_string(),
            token: "test-token".to_string(),
            description: String::new(),
            default_client: Some("mihomo".to_string()),
            template_id: None,
            group_id: None,
            enabled: true,
            expires_at: None,
            status: "active".to_string(),
            include_rules: true,
            portal_enabled: false,
            portal_slug: None,
            portal_access_code_set: false,
            node_group_ids: vec![10],
            node_ids: vec![1, 2],
            nodes,
            created_at: String::new(),
            updated_at: String::new(),
        };

        let response = export_mihomo(
            subscription,
            ExportMode::Strict,
            None,
            "mihomo",
            &ExportNodeMetadata {
                countries: &HashMap::new(),
                risks: &HashMap::new(),
            },
            2,
            3,
        )
        .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let yaml = String::from_utf8(body.to_vec()).unwrap();

        assert!(!yaml.contains("节点分组·"));
        assert!(yaml.contains("name: 故障转移"));
        assert!(yaml.contains("name: 负载均衡"));
    }

    #[tokio::test]
    async fn sing_box_export_uses_only_supported_root_fields() {
        let node = NodeView {
            id: 1,
            name: "SS node".to_string(),
            protocol: "shadowsocks".to_string(),
            raw_link: String::new(),
            server: "192.0.2.1".to_string(),
            port: 443,
            enabled: true,
            group_id: None,
            source_type: "manual".to_string(),
            source_ref: None,
            upstream_missing: false,
            fingerprint: "fingerprint-1".to_string(),
            settings: serde_json::json!({
                "method": "aes-128-gcm",
                "password": "test-password"
            }),
            remark: String::new(),
            last_latency_ms: None,
            last_latency_status: None,
            last_latency_message: None,
            last_latency_tested_at: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let subscription = SubscriptionView {
            id: 1,
            name: "sing-box subscription".to_string(),
            token: "test-token".to_string(),
            description: String::new(),
            default_client: Some("sing-box".to_string()),
            template_id: None,
            group_id: None,
            enabled: true,
            expires_at: None,
            status: "active".to_string(),
            include_rules: true,
            portal_enabled: false,
            portal_slug: None,
            portal_access_code_set: false,
            node_group_ids: Vec::new(),
            node_ids: vec![1],
            nodes: vec![node],
            created_at: String::new(),
            updated_at: String::new(),
        };

        let response = export_sing_box(subscription, ExportMode::Strict, None).unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let config = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
        let root = config.as_object().unwrap();

        assert!(!root.contains_key("title"));
        assert!(!root.contains_key("remarks"));
        assert!(root.contains_key("outbounds"));
        assert_eq!(
            config
                .pointer("/route/final")
                .and_then(|value| value.as_str()),
            Some("select")
        );
    }

    #[test]
    fn creates_country_load_balance_groups_from_verified_node_metadata() {
        let mut root = serde_yaml::from_str::<Value>(
            r#"
proxy-groups:
  - name: PROXY
    type: select
    proxies: [MANUAL, AUTO, DIRECT]
"#,
        )
        .unwrap()
        .as_mapping()
        .unwrap()
        .clone();
        let countries = HashMap::from([
            (1_i64, "JP".to_string()),
            (2_i64, "JP".to_string()),
            (3_i64, "JP".to_string()),
            (4_i64, "US".to_string()),
            (5_i64, "US".to_string()),
            (6_i64, "HK".to_string()),
            (7_i64, "GB".to_string()),
            (8_i64, "DE".to_string()),
        ]);
        let proxy_names = HashMap::from([
            (1_i64, "JP-01".to_string()),
            (2_i64, "JP-02".to_string()),
            (3_i64, "JP-03".to_string()),
            (4_i64, "US-01".to_string()),
            (5_i64, "US-02".to_string()),
            (6_i64, "HK-01".to_string()),
            (7_i64, "GB-01".to_string()),
            (8_i64, "DE-01".to_string()),
            (9_i64, "UNKNOWN-01".to_string()),
        ]);

        ensure_mihomo_country_load_balance_groups(&mut root, &countries, &proxy_names, 2, 3);

        let groups = root
            .get(Value::String("proxy-groups".to_string()))
            .and_then(Value::as_sequence)
            .unwrap();
        assert_eq!(groups.len(), 5);
        let japan = groups
            .iter()
            .find_map(|group| {
                let mapping = group.as_mapping()?;
                (mapping
                    .get(Value::String("name".to_string()))
                    .and_then(Value::as_str)
                    == Some("日本负载"))
                .then_some(mapping)
            })
            .unwrap();
        assert_eq!(
            japan
                .get(Value::String("type".to_string()))
                .and_then(Value::as_str),
            Some("load-balance")
        );
        assert_eq!(
            japan
                .get(Value::String("strategy".to_string()))
                .and_then(Value::as_str),
            Some("consistent-hashing")
        );
        let japan_fallback = groups
            .iter()
            .find_map(|group| {
                let mapping = group.as_mapping()?;
                (mapping
                    .get(Value::String("name".to_string()))
                    .and_then(Value::as_str)
                    == Some("日本故障转移"))
                .then_some(mapping)
            })
            .unwrap();
        assert_eq!(
            japan_fallback
                .get(Value::String("type".to_string()))
                .and_then(Value::as_str),
            Some("fallback")
        );
        assert_eq!(
            japan_fallback
                .get(Value::String("proxies".to_string()))
                .and_then(Value::as_sequence)
                .unwrap()
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>(),
            vec!["JP-01", "JP-02", "JP-03"]
        );
        assert_eq!(
            japan_fallback
                .get(Value::String("max-failed-times".to_string()))
                .and_then(Value::as_i64),
            Some(2)
        );
        let other_region_members = groups
            .iter()
            .find_map(|group| {
                let mapping = group.as_mapping()?;
                (mapping
                    .get(Value::String("name".to_string()))
                    .and_then(Value::as_str)
                    == Some("其他地区负载"))
                .then(|| {
                    mapping
                        .get(Value::String("proxies".to_string()))
                        .and_then(Value::as_sequence)
                        .unwrap()
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                })
            })
            .unwrap();
        assert_eq!(other_region_members, vec!["DE-01", "GB-01", "HK-01"]);
        for absent_group in [
            "香港负载",
            "香港故障转移",
            "美国故障转移",
            "其他地区故障转移",
        ] {
            assert!(!mihomo_proxy_group_exists(&root, absent_group));
        }
        let selector_members = groups[0]
            .as_mapping()
            .and_then(|mapping| mapping.get(Value::String("proxies".to_string())))
            .and_then(Value::as_sequence)
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();
        assert_eq!(
            selector_members,
            vec![
                "MANUAL",
                "AUTO",
                "日本负载",
                "日本故障转移",
                "美国负载",
                "其他地区负载",
                "DIRECT"
            ]
        );
    }

    #[test]
    fn rebuilds_reserved_fraud_groups_even_for_single_country_node() {
        let mut root = serde_yaml::from_str::<Value>(
            r#"
proxy-groups:
  - name: 代理选择
    type: select
    proxies: [日本故障转移0欺诈, DIRECT]
  - name: 日本故障转移0欺诈
    type: select
    proxies: [UNTRUSTED]
"#,
        )
        .unwrap()
        .as_mapping()
        .unwrap()
        .clone();
        let countries = HashMap::from([
            (1_i64, "JP".to_string()),
            (2_i64, "DE".to_string()),
            (3_i64, "KR".to_string()),
            (4_i64, "TR".to_string()),
        ]);
        let proxy_names = HashMap::from([
            (1_i64, "JP-01".to_string()),
            (2_i64, "DE-01".to_string()),
            (3_i64, "KR-01".to_string()),
            (4_i64, "TR-01".to_string()),
            (5_i64, "GLOBAL-RESIDENTIAL".to_string()),
        ]);
        let mut global_residential = risk_record(5, "low", 5);
        let mut global_traits = serde_json::from_str::<NodeRiskTraits>(
            global_residential.risk_traits_json.as_deref().unwrap(),
        )
        .unwrap();
        global_traits.conflicts.push("usage_type".to_string());
        global_residential.risk_traits_json = Some(serde_json::to_string(&global_traits).unwrap());
        let risks = HashMap::from([
            (1_i64, risk_record(1, "zero", 0)),
            (2_i64, risk_record(2, "low", 12)),
            (3_i64, risk_record(3, "zero", 0)),
            (4_i64, risk_record(4, "low", 12)),
            (5_i64, global_residential),
        ]);

        ensure_mihomo_fraud_groups(&mut root, &countries, &risks, &proxy_names);
        let yaml = serde_yaml::to_string(&root).unwrap();
        assert!(yaml.contains("name: 0欺诈"));
        assert!(yaml.contains("name: 低欺诈"));
        assert!(yaml.contains("name: 手动选择住宅网络"));
        assert!(yaml.contains("name: 日本故障转移0欺诈"));
        assert!(yaml.contains("name: 德国故障转移低欺诈"));
        assert!(yaml.contains("name: 韩国故障转移0欺诈"));
        assert!(yaml.contains("name: 土耳其故障转移低欺诈"));
        assert!(yaml.contains("JP-01"));
        assert!(yaml.contains("DE-01"));
        assert!(yaml.contains("KR-01"));
        assert!(yaml.contains("TR-01"));
        assert!(!yaml.contains("UNTRUSTED"));
        let residential = root
            .get(Value::String("proxy-groups".to_string()))
            .and_then(Value::as_sequence)
            .unwrap()
            .iter()
            .filter_map(Value::as_mapping)
            .find(|group| {
                group
                    .get(Value::String("name".to_string()))
                    .and_then(Value::as_str)
                    == Some("手动选择住宅网络")
            })
            .unwrap();
        assert_eq!(
            residential
                .get(Value::String("type".to_string()))
                .and_then(Value::as_str),
            Some("select")
        );
        assert!(
            residential
                .get(Value::String("proxies".to_string()))
                .and_then(Value::as_sequence)
                .is_some_and(
                    |proxies| proxies.contains(&Value::String("GLOBAL-RESIDENTIAL".to_string()))
                )
        );
    }

    #[test]
    fn excludes_unsafe_or_incomplete_nodes_from_strict_fraud_groups() {
        let mut root = Mapping::new();
        let proxy_names = HashMap::from([
            (1_i64, "SAFE".to_string()),
            (2_i64, "VPN".to_string()),
            (3_i64, "UNKNOWN".to_string()),
            (4_i64, "HIGH-ISP".to_string()),
        ]);
        let mut safe = risk_record(1, "zero", 0);
        let mut vpn = risk_record(2, "zero", 0);
        let mut vpn_traits =
            serde_json::from_str::<NodeRiskTraits>(vpn.risk_traits_json.as_deref().unwrap())
                .unwrap();
        vpn_traits.is_vpn = Some(true);
        vpn.risk_traits_json = Some(serde_json::to_string(&vpn_traits).unwrap());
        let mut unknown = risk_record(3, "zero", 0);
        unknown.risk_traits_json = None;
        let mut high_isp = risk_record(4, "zero", 0);
        high_isp.scamalytics_isp_risk_score = Some(10);
        safe.country_code = Some("JP".to_string());
        let risks = HashMap::from([(1, safe), (2, vpn), (3, unknown), (4, high_isp)]);

        ensure_mihomo_fraud_groups(&mut root, &HashMap::new(), &risks, &proxy_names);
        let yaml = serde_yaml::to_string(&root).unwrap();
        assert!(yaml.contains("SAFE"));
        assert!(!yaml.contains("VPN"));
        assert!(!yaml.contains("UNKNOWN"));
        assert!(!yaml.contains("HIGH-ISP"));
    }

    #[test]
    fn localizes_dynamic_mihomo_country_group_names() {
        assert_eq!(mihomo_country_label("KR"), "韩国");
        assert_eq!(mihomo_country_label("tr"), "土耳其");
        assert_eq!(mihomo_country_label(" DE "), "德国");
        assert_eq!(mihomo_country_label("ZZ"), "未知地区");
    }

    #[test]
    fn honors_configured_country_group_thresholds() {
        let mut root = serde_yaml::from_str::<Value>(
            r#"
proxy-groups:
  - name: PROXY
    type: select
    proxies: [MANUAL, AUTO, DIRECT]
"#,
        )
        .unwrap()
        .as_mapping()
        .unwrap()
        .clone();
        let countries = HashMap::from([
            (1_i64, "JP".to_string()),
            (2_i64, "JP".to_string()),
            (3_i64, "JP".to_string()),
            (4_i64, "US".to_string()),
            (5_i64, "US".to_string()),
        ]);
        let proxy_names = HashMap::from([
            (1_i64, "JP-01".to_string()),
            (2_i64, "JP-02".to_string()),
            (3_i64, "JP-03".to_string()),
            (4_i64, "US-01".to_string()),
            (5_i64, "US-02".to_string()),
        ]);

        ensure_mihomo_country_load_balance_groups(&mut root, &countries, &proxy_names, 4, 5);

        assert!(!mihomo_proxy_group_exists(&root, "日本负载"));
        assert!(!mihomo_proxy_group_exists(&root, "日本故障转移"));
        assert!(!mihomo_proxy_group_exists(&root, "美国负载"));
        assert!(mihomo_proxy_group_exists(&root, "其他地区负载"));
    }

    #[test]
    fn keeps_existing_mihomo_template_rules() {
        let value = serde_yaml::from_str::<Value>(
            r#"
proxy-groups:
  - name: Custom
    type: select
    proxies:
      - DIRECT
rules:
  - DOMAIN-SUFFIX,example.com,Custom
  - IP-CIDR,198.51.100.0/24,Custom,no-resolve
  - DST-PORT,8443,Custom
  - NETWORK,tcp,Custom
  - PROCESS-NAME,example-client,Custom
  - AND,((DOMAIN,example.com),(NETWORK,tcp)),Custom
  - OR,((DST-PORT,80),(DST-PORT,443)),Custom
  - NOT,((DOMAIN,blocked.example)),Custom
  - MATCH,Custom
"#,
        )
        .unwrap();
        let mut root = value.as_mapping().unwrap().clone();
        let original_rules = root
            .get(Value::String("rules".to_string()))
            .and_then(Value::as_sequence)
            .unwrap()
            .clone();

        ensure_mihomo_default_proxy_groups(&mut root, &["Proxy".to_string()]);
        ensure_mihomo_match_rule(&mut root);

        let rules = root
            .get(Value::String("rules".to_string()))
            .and_then(Value::as_sequence)
            .unwrap();
        assert_eq!(rules, &original_rules);
    }

    #[test]
    fn built_in_clash_template_avoids_geoip_database_dependency() {
        let template = super::built_in_clash_routing_template().unwrap();
        let parsed = serde_yaml::from_str::<Value>(template).unwrap();
        let root = parsed.as_mapping().unwrap();
        let groups = root
            .get(Value::String("proxy-groups".to_string()))
            .and_then(Value::as_sequence)
            .unwrap();

        assert!(!template.contains("GEOIP,CN"));
        assert!(!template.contains("fallback-filter:"));
        assert!(!template.contains("\n  fallback:\n"));
        assert_eq!(groups.len(), 5);
        assert!(template.contains("      - 手动选择\n      - 自动选择"));
        assert!(!template.contains("  - name: Ai平台"));
        assert!(!template.contains("  - name: 漏网之鱼"));
        assert!(template.contains("DOMAIN-SUFFIX,chatgpt.com,节点选择"));
        assert!(template.contains("DOMAIN-SUFFIX,github.com,节点选择"));
        assert!(template.contains("RULE-SET,proxygfw,节点选择"));
        assert!(template.contains("RULE-SET,banad,REJECT"));
        assert!(template.contains("RULE-SET,chinadomain,DIRECT"));
        assert!(template.contains("MATCH,节点选择"));
    }

    #[test]
    fn existing_builtin_network_defaults_are_fixed_without_touching_custom_templates() {
        for (kind, name, source) in [
            (
                "clash",
                BUILTIN_CLASH_TEMPLATE_NAME,
                crate::services::template_seed_service::CLASH_TEMPLATE,
            ),
            (
                "mihomo",
                BUILTIN_MIHOMO_TEMPLATE_NAME,
                crate::services::template_seed_service::MIHOMO_TEMPLATE,
            ),
        ] {
            let mut template = sample_template(kind, source);
            template.name = name.into();
            // Simulate a stored pre-fix record rather than relying on new seed data.
            let mut stored: Value = serde_yaml::from_str(source).unwrap();
            stored["dns"] = serde_yaml::from_str("nameserver: [192.0.2.1]").unwrap();
            template.content = serde_yaml::to_string(&stored).unwrap();
            let root = super::parse_yaml_template(Some(&template), kind).unwrap();
            assert_eq!(root.get("ipv6"), Some(&Value::Bool(false)));
            assert_ne!(root.get("dns"), Some(&stored["dns"]));
            let rules = root.get("rules").unwrap().as_sequence().unwrap();
            assert!(
                rules
                    .iter()
                    .any(|rule| rule.as_str() == Some("DOMAIN-SUFFIX,cn,DIRECT"))
            );
            if kind == "mihomo" {
                let dns = root.get("dns").unwrap();
                assert_eq!(
                    dns["direct-nameserver-follow-policy"].as_bool(),
                    Some(false)
                );
                assert_eq!(dns["direct-nameserver"][0].as_str(), Some("223.5.5.5"));
                assert!(
                    rules
                        .iter()
                        .any(|rule| rule.as_str() == Some("RULE-SET,cn_ip,DIRECT"))
                );
                assert!(
                    !rules
                        .iter()
                        .any(|rule| rule.as_str() == Some("RULE-SET,cn_ip,DIRECT,no-resolve"))
                );
            } else {
                for provider in root
                    .get("rule-providers")
                    .unwrap()
                    .as_mapping()
                    .unwrap()
                    .values()
                {
                    assert_eq!(provider["format"].as_str(), Some("text"));
                }
            }
            template.is_builtin = 0;
            let custom = super::parse_yaml_template(Some(&template), kind).unwrap();
            assert_eq!(Value::Mapping(custom), stored);
        }
    }

    #[tokio::test]
    #[ignore = "generates synthetic exports for manual core validation"]
    async fn generate_routing_smoke_exports() {
        let directory = std::path::Path::new("../output/routing-smoke");
        std::fs::create_dir_all(directory).unwrap();
        for (kind, name, content) in [
            (
                "clash",
                BUILTIN_CLASH_TEMPLATE_NAME,
                crate::services::template_seed_service::CLASH_TEMPLATE,
            ),
            (
                "mihomo",
                BUILTIN_MIHOMO_TEMPLATE_NAME,
                crate::services::template_seed_service::MIHOMO_TEMPLATE,
            ),
        ] {
            let mut template = sample_template(kind, content);
            template.name = name.into();
            let body = response_body(
                export_mihomo(
                    sample_shadowsocks_subscription(kind),
                    ExportMode::Strict,
                    Some(&template),
                    kind,
                    &ExportNodeMetadata {
                        countries: &HashMap::new(),
                        risks: &HashMap::new(),
                    },
                    2,
                    3,
                )
                .unwrap(),
            )
            .await;
            let nodes_only = super::nodes_only_export(&body, "mihomo").unwrap();
            let parsed: Value = serde_yaml::from_str(&nodes_only).unwrap();
            let expected_group = if kind == "clash" {
                "节点选择"
            } else {
                "代理选择"
            };
            assert_eq!(
                parsed["rules"],
                serde_yaml::to_value(vec![format!("MATCH,{expected_group}")]).unwrap()
            );
            std::fs::write(directory.join(format!("{kind}-nodes.yaml")), nodes_only).unwrap();
            std::fs::write(directory.join(format!("{kind}.yaml")), body).unwrap();
        }
    }

    #[test]
    fn built_in_mihomo_template_prefers_manual_and_has_common_site_rules() {
        let template = crate::services::template_seed_service::MIHOMO_TEMPLATE;
        let parsed = serde_yaml::from_str::<Value>(template).unwrap();

        assert!(parsed.as_mapping().is_some());
        let root = parsed.as_mapping().unwrap();
        let hosts = root
            .get(Value::String("hosts".to_string()))
            .and_then(Value::as_mapping)
            .unwrap();
        let dns = root
            .get(Value::String("dns".to_string()))
            .and_then(Value::as_mapping)
            .unwrap();
        let nameserver_policy = dns
            .get(Value::String("nameserver-policy".to_string()))
            .and_then(Value::as_mapping)
            .unwrap();

        assert!(hosts.is_empty());
        assert_eq!(
            dns.get(Value::String("use-hosts".to_string()))
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            dns.get(Value::String("ipv6".to_string()))
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            dns.get(Value::String("respect-rules".to_string()))
                .and_then(Value::as_bool),
            Some(false)
        );
        assert!(nameserver_policy.contains_key(Value::String("rule-set:cn_domain".to_string())));
        assert!(
            nameserver_policy
                .contains_key(Value::String("rule-set:geolocation-not-cn".to_string()))
        );
        let nameservers = dns
            .get(Value::String("nameserver".to_string()))
            .and_then(Value::as_sequence)
            .unwrap();
        assert_eq!(
            nameservers.first().and_then(Value::as_str),
            Some("https://1.1.1.1/dns-query#代理选择")
        );
        assert!(template.contains("  proxy-server-nameserver:"));
        let proxy_server_nameservers = dns
            .get(Value::String("proxy-server-nameserver".to_string()))
            .and_then(Value::as_sequence)
            .unwrap();
        assert_eq!(proxy_server_nameservers.len(), 2);
        assert!(proxy_server_nameservers.iter().all(|server| {
            matches!(
                server.as_str(),
                Some("https://dns.alidns.com/dns-query" | "https://doh.pub/dns-query")
            )
        }));
        let fake_ip_filter = dns
            .get("fake-ip-filter")
            .and_then(Value::as_sequence)
            .unwrap();
        for domain in [
            "rule-set:private_domain",
            "rule-set:cn_domain",
            "+.cn",
            "+.lan",
        ] {
            assert!(
                fake_ip_filter
                    .iter()
                    .any(|value| value.as_str() == Some(domain))
            );
        }
        assert!(template.contains(
            "      - 手动选择\n      - 自动选择\n      - 故障转移\n      - 负载均衡\n      - DIRECT"
        ));
        let groups = root
            .get(Value::String("proxy-groups".to_string()))
            .and_then(Value::as_sequence)
            .unwrap();
        assert_eq!(groups.len(), 5);
        assert!(!template.contains("  - name: 流媒体"));
        assert!(!template.contains("  - name: 谷歌服务"));
        assert!(!template.contains("  - name: 国内网站"));
        assert!(!template.contains("  - name: 下载拦截"));
        assert!(template.contains("DOMAIN-SUFFIX,chatgpt.com,代理选择"));
        assert!(template.contains("DOMAIN-SUFFIX,github.com,代理选择"));
        assert!(template.contains("DOMAIN-SUFFIX,steamcommunity.com,代理选择"));
        assert!(template.contains("RULE-SET,google_domain,代理选择"));
        assert!(template.contains("RULE-SET,cn_domain,DIRECT"));
        assert!(template.contains("PROCESS-NAME-WILDCARD,*torrent*,REJECT"));
        assert!(template.contains("RULE-SET,tracker_domain,REJECT"));
        assert!(template.contains("DST-PORT,6881-6999,REJECT"));
        assert!(template.contains("MATCH,代理选择"));
        assert!(!template.contains("  - name: YOUTUBE"));
        assert!(!template.contains("  - name: NETFLIX"));
        assert!(!template.contains("  - name: MEDIA"));
    }

    #[test]
    fn compacts_only_legacy_persisted_builtin_policy_groups() {
        let content = r#"proxy-groups:
  - name: 节点选择
    type: select
    proxies: [手动切换, Ai平台, 全球直连, 广告拦截, 漏网之鱼]
  - name: 手动切换
    type: select
    proxies: [DIRECT]
  - name: Ai平台
    type: select
    proxies: [节点选择]
  - name: 全球直连
    type: select
    proxies: [DIRECT]
  - name: 广告拦截
    type: select
    proxies: [REJECT]
  - name: 漏网之鱼
    type: select
    proxies: [节点选择]
rules:
  - DOMAIN-SUFFIX,example.com,Ai平台
  - RULE-SET,localarea,全球直连,no-resolve
  - RULE-SET,banad,广告拦截
  - MATCH,漏网之鱼
"#;
        let mut template = sample_template("clash", content);
        template.name = "Built-in Clash ACL4SSR Style".to_string();
        let mut root = serde_yaml::from_str::<Value>(content)
            .unwrap()
            .as_mapping()
            .unwrap()
            .clone();

        compact_builtin_mihomo_policy_groups(&mut root, Some(&template));
        ensure_mihomo_default_proxy_groups(&mut root, &["Proxy".to_string()]);

        for removed in ["手动切换", "Ai平台", "全球直连", "广告拦截", "漏网之鱼"]
        {
            assert!(!mihomo_proxy_group_exists(&root, removed));
        }
        for retained in ["节点选择", "手动选择", "自动选择", "故障转移", "负载均衡"]
        {
            assert!(mihomo_proxy_group_exists(&root, retained));
        }
        let rendered = serde_yaml::to_string(&root).unwrap();
        assert!(rendered.contains("DOMAIN-SUFFIX,example.com,节点选择"));
        assert!(rendered.contains("RULE-SET,localarea,DIRECT,no-resolve"));
        assert!(rendered.contains("RULE-SET,banad,REJECT"));
        assert!(rendered.contains("MATCH,节点选择"));

        let legacy_mihomo_content = r#"proxy-groups:
  - name: PROXY
    type: select
    proxies: [MANUAL, AUTO, AI, DOMESTIC, DOWNLOAD-BLOCK, FINAL]
  - name: MANUAL
    type: select
    proxies: [DIRECT]
  - name: AI
    type: select
    proxies: [PROXY]
  - name: DOMESTIC
    type: select
    proxies: [DIRECT]
  - name: DOWNLOAD-BLOCK
    type: select
    proxies: [REJECT]
  - name: FINAL
    type: select
    proxies: [PROXY]
rules:
  - RULE-SET,ai,AI
  - RULE-SET,cn_domain,DOMESTIC,no-resolve
  - PROCESS-NAME,qbittorrent,DOWNLOAD-BLOCK
  - MATCH,FINAL
"#;
        let mut legacy_mihomo_template = sample_template("mihomo", legacy_mihomo_content);
        legacy_mihomo_template.name = "Built-in Mihomo Policy Orchestrator".to_string();
        let mut legacy_mihomo_root = serde_yaml::from_str::<Value>(legacy_mihomo_content)
            .unwrap()
            .as_mapping()
            .unwrap()
            .clone();
        compact_builtin_mihomo_policy_groups(
            &mut legacy_mihomo_root,
            Some(&legacy_mihomo_template),
        );
        ensure_mihomo_default_proxy_groups(&mut legacy_mihomo_root, &["Proxy".to_string()]);
        let rendered = serde_yaml::to_string(&legacy_mihomo_root).unwrap();
        assert!(mihomo_proxy_group_exists(&legacy_mihomo_root, "代理选择"));
        assert!(mihomo_proxy_group_exists(&legacy_mihomo_root, "手动选择"));
        assert!(!mihomo_proxy_group_exists(&legacy_mihomo_root, "AI"));
        assert!(!mihomo_proxy_group_exists(&legacy_mihomo_root, "FINAL"));
        assert!(rendered.contains("RULE-SET,ai,代理选择"));
        assert!(rendered.contains("RULE-SET,cn_domain,DIRECT,no-resolve"));
        assert!(rendered.contains("PROCESS-NAME,qbittorrent,REJECT"));
        assert!(rendered.contains("MATCH,代理选择"));

        template.is_builtin = 0;
        let mut custom_root = serde_yaml::from_str::<Value>(content)
            .unwrap()
            .as_mapping()
            .unwrap()
            .clone();
        compact_builtin_mihomo_policy_groups(&mut custom_root, Some(&template));
        assert!(mihomo_proxy_group_exists(&custom_root, "Ai平台"));
        assert!(mihomo_proxy_group_exists(&custom_root, "漏网之鱼"));
    }
}
