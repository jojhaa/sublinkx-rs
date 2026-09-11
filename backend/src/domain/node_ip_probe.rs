use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct NodeRiskTraits {
    pub usage_type: Option<String>,
    pub is_proxy: Option<bool>,
    pub is_vpn: Option<bool>,
    pub is_tor: Option<bool>,
    pub is_hosting: Option<bool>,
    pub is_abuser: Option<bool>,
    pub is_relay: Option<bool>,
    pub threat_level: Option<String>,
    pub is_botnet_c2: Option<bool>,
    #[serde(default)]
    pub conflicts: Vec<String>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct NodeIpProbeRecord {
    pub node_id: i64,
    pub status: String,
    pub ip: Option<String>,
    pub ip_version: Option<i64>,
    pub exit_ip_revision: i64,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub country_source: Option<String>,
    pub intelligence_status: Option<String>,
    pub intelligence_message: Option<String>,
    pub risk_ip: Option<String>,
    pub risk_status: Option<String>,
    pub scamalytics_fraud_score: Option<i64>,
    pub scamalytics_isp_risk_score: Option<i64>,
    pub risk_checked_at: Option<String>,
    pub risk_expires_at_unix_ms: Option<i64>,
    pub risk_message: Option<String>,
    #[serde(skip_serializing)]
    pub risk_traits_json: Option<String>,
    #[serde(skip_serializing)]
    pub risk_traits_expires_at_unix_ms: Option<i64>,
    pub message: Option<String>,
    pub probed_at: String,
    pub country_updated_at: Option<String>,
    pub intelligence_updated_at: Option<String>,
    pub updated_at: String,
}

impl NodeIpProbeRecord {
    pub fn qualifies_for_zero_fraud(&self, now_unix_ms: i64) -> bool {
        self.current_scores_and_traits(now_unix_ms)
            .is_some_and(|traits| {
                self.scamalytics_fraud_score == Some(0)
                    && self.scamalytics_isp_risk_score == Some(0)
                    && traits.has_clean_base_network()
            })
    }

    pub fn qualifies_for_low_fraud(&self, now_unix_ms: i64) -> bool {
        self.current_scores_and_traits(now_unix_ms)
            .is_some_and(|traits| {
                matches!(self.scamalytics_fraud_score, Some(1..=20))
                    && matches!(self.scamalytics_isp_risk_score, Some(0..=20))
                    && traits.has_clean_base_network()
            })
    }

    pub fn qualifies_for_residential(&self, now_unix_ms: i64) -> bool {
        self.current_scores_and_traits(now_unix_ms)
            .is_some_and(|traits| {
                matches!(self.scamalytics_fraud_score, Some(0..=9))
                    && matches!(self.scamalytics_isp_risk_score, Some(0..=9))
                    && traits.is_residential()
            })
    }

    fn current_scores_and_traits(&self, now_unix_ms: i64) -> Option<NodeRiskTraits> {
        if self.status != "ok"
            || self.risk_ip != self.ip
            || !self
                .risk_expires_at_unix_ms
                .is_some_and(|expires| expires > now_unix_ms)
            || !self
                .risk_traits_expires_at_unix_ms
                .is_some_and(|expires| expires > now_unix_ms)
        {
            return None;
        }
        serde_json::from_str(self.risk_traits_json.as_deref()?).ok()
    }
}

impl NodeRiskTraits {
    fn has_clean_base_network(&self) -> bool {
        !self.has_conflict(&["is_abuser", "is_relay", "is_tor", "is_vpn"])
            && self.is_abuser == Some(false)
            && self.is_relay == Some(false)
            && self.is_tor == Some(false)
            && self.is_vpn == Some(false)
    }

    fn is_residential(&self) -> bool {
        self.has_clean_base_network()
            && !self.has_conflict(&["is_proxy", "is_hosting", "is_botnet_c2", "threat_level"])
            && self.usage_type.as_deref() == Some("residential")
            && self.is_proxy == Some(false)
            && self.is_hosting == Some(false)
            && self.is_botnet_c2 != Some(true)
            && matches!(self.threat_level.as_deref(), None | Some("informational"))
    }

    fn has_conflict(&self, fields: &[&str]) -> bool {
        self.conflicts
            .iter()
            .any(|conflict| fields.contains(&conflict.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(fraud_score: i64, isp_risk_score: i64, traits: NodeRiskTraits) -> NodeIpProbeRecord {
        NodeIpProbeRecord {
            node_id: 1,
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
            risk_status: Some("zero".to_string()),
            scamalytics_fraud_score: Some(fraud_score),
            scamalytics_isp_risk_score: Some(isp_risk_score),
            risk_checked_at: None,
            risk_expires_at_unix_ms: Some(2_000),
            risk_message: None,
            risk_traits_json: Some(serde_json::to_string(&traits).unwrap()),
            risk_traits_expires_at_unix_ms: Some(2_000),
            message: None,
            probed_at: String::new(),
            country_updated_at: None,
            intelligence_updated_at: None,
            updated_at: String::new(),
        }
    }

    fn clean_traits() -> NodeRiskTraits {
        NodeRiskTraits {
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
        }
    }

    #[test]
    fn strict_fraud_groups_require_fresh_scores_and_explicit_clean_traits() {
        let zero = record(0, 0, clean_traits());
        assert!(zero.qualifies_for_zero_fraud(1_000));
        assert!(zero.qualifies_for_residential(1_000));

        let low = record(12, 8, clean_traits());
        assert!(low.qualifies_for_low_fraud(1_000));
        assert!(!low.qualifies_for_residential(1_000));

        let mut unknown_vpn = clean_traits();
        unknown_vpn.is_vpn = None;
        assert!(!record(0, 0, unknown_vpn).qualifies_for_zero_fraud(1_000));

        let mut conflicted = clean_traits();
        conflicted.conflicts.push("is_tor".to_string());
        assert!(!record(0, 0, conflicted).qualifies_for_zero_fraud(1_000));

        assert!(!zero.qualifies_for_zero_fraud(2_000));
    }

    #[test]
    fn residential_group_rejects_proxy_hosting_botnet_and_threats() {
        let mut proxy = clean_traits();
        proxy.is_proxy = Some(true);
        assert!(!record(5, 5, proxy).qualifies_for_residential(1_000));

        let mut hosting = clean_traits();
        hosting.is_hosting = Some(true);
        assert!(!record(5, 5, hosting).qualifies_for_residential(1_000));

        let mut botnet = clean_traits();
        botnet.is_botnet_c2 = Some(true);
        assert!(!record(5, 5, botnet).qualifies_for_residential(1_000));

        let mut threatened = clean_traits();
        threatened.threat_level = Some("low".to_string());
        assert!(!record(5, 5, threatened).qualifies_for_residential(1_000));
    }

    #[test]
    fn residential_group_uses_normalized_usage_type_across_source_disagreement() {
        let mut normalized_residential = clean_traits();
        normalized_residential
            .conflicts
            .push("usage_type".to_string());
        assert!(record(5, 5, normalized_residential).qualifies_for_residential(1_000));

        let mut vpn_conflict = clean_traits();
        vpn_conflict.conflicts.push("is_vpn".to_string());
        assert!(!record(5, 5, vpn_conflict).qualifies_for_residential(1_000));
    }
}
