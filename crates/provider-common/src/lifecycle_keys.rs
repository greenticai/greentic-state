use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderProvenance {
    pub describe_hash: String,
    pub artifact_digest: String,
    pub schema_hash: String,
}

pub fn messaging_config_key(provider_id: &str, tenant_id: &str, team_id: Option<&str>) -> String {
    let mut key = base_key(provider_id, tenant_id, team_id);
    key.push_str(":config");
    key
}

pub fn messaging_provenance_key(
    provider_id: &str,
    tenant_id: &str,
    team_id: Option<&str>,
) -> String {
    let mut key = base_key(provider_id, tenant_id, team_id);
    key.push_str(":provenance");
    key
}

pub fn messaging_state_key(
    provider_id: &str,
    tenant_id: &str,
    team_id: Option<&str>,
    state_name: &str,
) -> String {
    let mut key = base_key(provider_id, tenant_id, team_id);
    key.push_str(":state:");
    key.push_str(state_name.trim());
    key
}

/// Known pre-0.6 key candidates to probe during best-effort migration.
pub fn legacy_messaging_config_keys(
    provider_id: &str,
    tenant_id: &str,
    team_id: Option<&str>,
) -> Vec<String> {
    legacy_base_keys(provider_id, tenant_id, team_id)
        .into_iter()
        .map(|base| format!("{base}:config"))
        .collect()
}

/// Known pre-0.6 provenance key candidates to probe during best-effort migration.
pub fn legacy_messaging_provenance_keys(
    provider_id: &str,
    tenant_id: &str,
    team_id: Option<&str>,
) -> Vec<String> {
    legacy_base_keys(provider_id, tenant_id, team_id)
        .into_iter()
        .map(|base| format!("{base}:provenance"))
        .collect()
}

fn base_key(provider_id: &str, tenant_id: &str, team_id: Option<&str>) -> String {
    let provider = provider_id.trim();
    let tenant = tenant_id.trim();
    let mut key = format!("providers:messaging:{provider}:tenants:{tenant}");
    if let Some(team) = team_id.map(str::trim).filter(|value| !value.is_empty()) {
        key.push_str(":teams:");
        key.push_str(team);
    }
    key
}

fn legacy_base_keys(provider_id: &str, tenant_id: &str, team_id: Option<&str>) -> Vec<String> {
    let provider = provider_id.trim();
    let tenant = tenant_id.trim();
    let team = team_id.map(str::trim).filter(|value| !value.is_empty());

    let mut bases = vec![
        format!("providers:{provider}:tenants:{tenant}"),
        format!("messaging:{provider}:tenants:{tenant}"),
        format!("messaging:{provider}:tenant:{tenant}"),
    ];
    if let Some(team) = team {
        bases.push(format!(
            "providers:{provider}:tenants:{tenant}:teams:{team}"
        ));
        bases.push(format!(
            "messaging:{provider}:tenants:{tenant}:teams:{team}"
        ));
        bases.push(format!("messaging:{provider}:tenant:{tenant}:team:{team}"));
    }
    bases
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_keys_without_team_scope() {
        assert_eq!(
            messaging_config_key("slack", "tenant-a", None),
            "providers:messaging:slack:tenants:tenant-a:config"
        );
        assert_eq!(
            messaging_provenance_key("slack", "tenant-a", None),
            "providers:messaging:slack:tenants:tenant-a:provenance"
        );
        assert_eq!(
            messaging_state_key("slack", "tenant-a", None, "session"),
            "providers:messaging:slack:tenants:tenant-a:state:session"
        );
    }

    #[test]
    fn builds_keys_with_team_scope() {
        assert_eq!(
            messaging_config_key("slack", "tenant-a", Some("team-1")),
            "providers:messaging:slack:tenants:tenant-a:teams:team-1:config"
        );
        assert_eq!(
            messaging_provenance_key("slack", "tenant-a", Some("team-1")),
            "providers:messaging:slack:tenants:tenant-a:teams:team-1:provenance"
        );
        assert_eq!(
            messaging_state_key("slack", "tenant-a", Some("team-1"), "session"),
            "providers:messaging:slack:tenants:tenant-a:teams:team-1:state:session"
        );
    }

    #[test]
    fn empty_team_is_ignored() {
        let no_team = messaging_config_key("slack", "tenant-a", None);
        let empty_team = messaging_config_key("slack", "tenant-a", Some("  "));
        assert_eq!(no_team, empty_team);
    }

    #[test]
    fn exposes_legacy_key_candidates_without_team() {
        let keys = legacy_messaging_config_keys("slack", "tenant-a", None);
        assert!(keys.contains(&"providers:slack:tenants:tenant-a:config".to_string()));
        assert!(keys.contains(&"messaging:slack:tenants:tenant-a:config".to_string()));
        assert!(keys.contains(&"messaging:slack:tenant:tenant-a:config".to_string()));
        assert!(!keys.iter().any(|key| key.contains(":teams:")));
    }

    #[test]
    fn exposes_legacy_key_candidates_with_team() {
        let keys = legacy_messaging_provenance_keys("slack", "tenant-a", Some("ops"));
        assert!(
            keys.contains(&"providers:slack:tenants:tenant-a:teams:ops:provenance".to_string())
        );
        assert!(
            keys.contains(&"messaging:slack:tenants:tenant-a:teams:ops:provenance".to_string())
        );
        assert!(keys.contains(&"messaging:slack:tenant:tenant-a:team:ops:provenance".to_string()));
    }
}
