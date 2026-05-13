//! Signed agent identity verification for ME-JEPA write tools.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Result as AnyhowResult};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;

pub(crate) const ANONYMOUS_AGENT_ID: &str = "anonymous";
pub(crate) const ENV_MEJEPA_AGENTS_CONFIG: &str = "CONTEXTGRAPH_MEJEPA_AGENTS_CONFIG";
pub(crate) const MEJEPA_AGENT_IDENTITY_UNVERIFIED: &str = "MEJEPA_AGENT_IDENTITY_UNVERIFIED";
pub(crate) const MEJEPA_AGENT_IDENTITY_CONFIG_INVALID: &str =
    "MEJEPA_AGENT_IDENTITY_CONFIG_INVALID";

const MAX_CLOCK_SKEW_MS: u64 = 5 * 60 * 1000;
const MIN_PSK_BYTES: usize = 16;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct IdentityAttestationRequest {
    pub(crate) session_id: String,
    pub(crate) nonce: String,
    pub(crate) timestamp_unix_ms: i64,
    pub(crate) signature_hex: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedAgentIdentity {
    pub(crate) id: String,
    pub(crate) authenticated: bool,
    pub(crate) config_path: Option<PathBuf>,
    pub(crate) session_id: Option<String>,
    pub(crate) nonce: Option<String>,
}

impl ResolvedAgentIdentity {
    pub(crate) fn anonymous() -> Self {
        Self {
            id: ANONYMOUS_AGENT_ID.to_string(),
            authenticated: false,
            config_path: None,
            session_id: None,
            nonce: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentsConfig {
    agents: Vec<ConfiguredAgent>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfiguredAgent {
    agent_id: String,
    psk: String,
    #[serde(default)]
    can_operator_override: bool,
}

pub(crate) fn resolve_feedback_identity(
    claimed_agent_id: Option<&str>,
    attestation: Option<&IdentityAttestationRequest>,
    tool_name: &str,
    now_unix_ms: i64,
) -> AnyhowResult<ResolvedAgentIdentity> {
    let claimed = claimed_agent_id.unwrap_or(ANONYMOUS_AGENT_ID).trim();
    if claimed.is_empty() {
        fail_identity("agentId must be non-empty when provided")?;
    }
    if claimed == ANONYMOUS_AGENT_ID && attestation.is_none() {
        return Ok(ResolvedAgentIdentity::anonymous());
    }
    let attestation = attestation.ok_or_else(|| {
        anyhow!(
            "{MEJEPA_AGENT_IDENTITY_UNVERIFIED}: non-anonymous feedback requires identityAttestation"
        )
    })?;
    verify_signed_identity(claimed, attestation, tool_name, false, now_unix_ms)
}

pub(crate) fn resolve_operator_identity(
    operator_id: &str,
    attestation: Option<&IdentityAttestationRequest>,
    tool_name: &str,
    now_unix_ms: i64,
) -> AnyhowResult<ResolvedAgentIdentity> {
    let claimed = operator_id.trim();
    if claimed.is_empty() || claimed == ANONYMOUS_AGENT_ID {
        fail_identity("operatorId must name a configured non-anonymous operator")?;
    }
    let attestation = attestation.ok_or_else(|| {
        anyhow!(
            "{MEJEPA_AGENT_IDENTITY_UNVERIFIED}: operator override requires identityAttestation"
        )
    })?;
    verify_signed_identity(claimed, attestation, tool_name, true, now_unix_ms)
}

#[cfg(test)]
pub(crate) fn sign_identity_attestation(
    psk: &str,
    tool_name: &str,
    claimed_id: &str,
    session_id: &str,
    nonce: &str,
    timestamp_unix_ms: i64,
) -> AnyhowResult<String> {
    let mut mac = HmacSha256::new_from_slice(psk.as_bytes())
        .map_err(|err| anyhow!("failed to construct HMAC: {err}"))?;
    mac.update(
        canonical_identity_message(tool_name, claimed_id, session_id, nonce, timestamp_unix_ms)
            .as_bytes(),
    );
    Ok(hex::encode(mac.finalize().into_bytes()))
}

fn verify_signed_identity(
    claimed_id: &str,
    attestation: &IdentityAttestationRequest,
    tool_name: &str,
    require_operator_override: bool,
    now_unix_ms: i64,
) -> AnyhowResult<ResolvedAgentIdentity> {
    validate_attestation_shape(attestation, now_unix_ms)?;
    let (config_path, agents) = load_agents_config()?;
    let configured = agents.get(claimed_id).ok_or_else(|| {
        anyhow!("{MEJEPA_AGENT_IDENTITY_UNVERIFIED}: {claimed_id:?} is not configured")
    })?;
    if require_operator_override && !configured.can_operator_override {
        fail_identity("configured identity is not allowed to record operator overrides")?;
    }
    let provided = hex::decode(attestation.signature_hex.trim()).map_err(|err| {
        anyhow!("{MEJEPA_AGENT_IDENTITY_UNVERIFIED}: signatureHex is not valid hex: {err}")
    })?;
    if provided.len() != 32 {
        fail_identity("signatureHex must decode to a 32-byte HMAC-SHA256 digest")?;
    }
    let mut mac = HmacSha256::new_from_slice(configured.psk.as_bytes())
        .map_err(|err| anyhow!("{MEJEPA_AGENT_IDENTITY_CONFIG_INVALID}: invalid PSK: {err}"))?;
    mac.update(
        canonical_identity_message(
            tool_name,
            claimed_id,
            &attestation.session_id,
            &attestation.nonce,
            attestation.timestamp_unix_ms,
        )
        .as_bytes(),
    );
    mac.verify_slice(&provided)
        .map_err(|_| anyhow!("{MEJEPA_AGENT_IDENTITY_UNVERIFIED}: signature mismatch"))?;
    Ok(ResolvedAgentIdentity {
        id: claimed_id.to_string(),
        authenticated: true,
        config_path: Some(config_path),
        session_id: Some(attestation.session_id.clone()),
        nonce: Some(attestation.nonce.clone()),
    })
}

fn canonical_identity_message(
    tool_name: &str,
    claimed_id: &str,
    session_id: &str,
    nonce: &str,
    timestamp_unix_ms: i64,
) -> String {
    format!(
        "mejepa-agent-identity-v1\n{tool_name}\n{claimed_id}\n{session_id}\n{nonce}\n{timestamp_unix_ms}"
    )
}

fn validate_attestation_shape(
    attestation: &IdentityAttestationRequest,
    now_unix_ms: i64,
) -> AnyhowResult<()> {
    if attestation.session_id.trim().is_empty() {
        fail_identity("sessionId must be non-empty")?;
    }
    if attestation.nonce.trim().is_empty() {
        fail_identity("nonce must be non-empty")?;
    }
    if attestation.timestamp_unix_ms <= 0 {
        fail_identity("timestampUnixMs must be positive")?;
    }
    if now_unix_ms.abs_diff(attestation.timestamp_unix_ms) > MAX_CLOCK_SKEW_MS {
        fail_identity("timestampUnixMs is outside the accepted 5 minute clock-skew window")?;
    }
    Ok(())
}

fn load_agents_config() -> AnyhowResult<(PathBuf, BTreeMap<String, ConfiguredAgent>)> {
    let path = agents_config_path()?;
    let bytes = fs::read(&path).map_err(|err| {
        anyhow!(
            "{MEJEPA_AGENT_IDENTITY_UNVERIFIED}: failed to read {}: {err}",
            path.display()
        )
    })?;
    let text = std::str::from_utf8(&bytes).map_err(|err| {
        anyhow!(
            "{MEJEPA_AGENT_IDENTITY_CONFIG_INVALID}: {} is not UTF-8: {err}",
            path.display()
        )
    })?;
    let parsed: AgentsConfig = toml::from_str(text).map_err(|err| {
        anyhow!(
            "{MEJEPA_AGENT_IDENTITY_CONFIG_INVALID}: failed to parse {}: {err}",
            path.display()
        )
    })?;
    if parsed.agents.is_empty() {
        bail!("{MEJEPA_AGENT_IDENTITY_CONFIG_INVALID}: agents.toml declares no agents");
    }
    let mut agents = BTreeMap::new();
    for agent in parsed.agents {
        validate_configured_agent(&agent)?;
        if agents
            .insert(agent.agent_id.clone(), agent.clone())
            .is_some()
        {
            bail!(
                "{MEJEPA_AGENT_IDENTITY_CONFIG_INVALID}: duplicate agent_id {:?}",
                agent.agent_id
            );
        }
    }
    Ok((path, agents))
}

fn agents_config_path() -> AnyhowResult<PathBuf> {
    if let Ok(path) = std::env::var(ENV_MEJEPA_AGENTS_CONFIG) {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            bail!("{MEJEPA_AGENT_IDENTITY_CONFIG_INVALID}: {ENV_MEJEPA_AGENTS_CONFIG} is empty");
        }
        return Ok(PathBuf::from(trimmed));
    }
    dirs::config_dir()
        .map(|root| root.join("mejepa").join("agents.toml"))
        .ok_or_else(|| {
            anyhow!(
                "{MEJEPA_AGENT_IDENTITY_UNVERIFIED}: could not resolve ~/.config/mejepa/agents.toml"
            )
        })
}

fn validate_configured_agent(agent: &ConfiguredAgent) -> AnyhowResult<()> {
    if agent.agent_id.trim().is_empty() || agent.agent_id != agent.agent_id.trim() {
        bail!("{MEJEPA_AGENT_IDENTITY_CONFIG_INVALID}: agent_id must be trimmed non-empty text");
    }
    if agent.agent_id == ANONYMOUS_AGENT_ID {
        bail!("{MEJEPA_AGENT_IDENTITY_CONFIG_INVALID}: anonymous is reserved");
    }
    if agent.agent_id.len() > 256 || agent.agent_id.chars().any(char::is_control) {
        bail!(
            "{MEJEPA_AGENT_IDENTITY_CONFIG_INVALID}: agent_id must be single-line text <=256 bytes"
        );
    }
    if agent.psk.len() < MIN_PSK_BYTES {
        bail!("{MEJEPA_AGENT_IDENTITY_CONFIG_INVALID}: psk must be at least 16 bytes");
    }
    Ok(())
}

fn fail_identity<T>(message: &str) -> AnyhowResult<T> {
    bail!("{MEJEPA_AGENT_IDENTITY_UNVERIFIED}: {message}")
}
