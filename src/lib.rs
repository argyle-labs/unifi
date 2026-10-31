//! unifi plugin — UniFi Network Controller driven over its local Integration API.
//!
//! A DUAL-facet plugin: one `Plugin` builder chain registers BOTH the
//! [`UnifiBackend`] (generic `service.*` lifecycle) AND the `unifi.`
//! `#[orca_tool]` surface (sites/devices/clients, read-only). The builder emits
//! all the wire dispatch, so the plugin hand-writes no op strings and owns no
//! runtime; it reaches orca only through the socket.
//!
//! The wire surface is the toolkit's cap-backed HTTP client (`delegated-http`),
//! so every request rides orca's `http.request` capability and this plugin links
//! no reqwest/rustls. Auth is a local API key sent as the `X-API-KEY` header —
//! the stateless scheme UniFi's official Network Integration API
//! (`/proxy/network/integration/v1`) uses on both UniFi OS consoles (UDM/Cloud
//! Key Gen2+) and self-hosted Network Application 8.x+. Older self-hosted
//! controllers predating that API only support cookie-session login
//! (`POST /api/login`); that variant is NOT implemented here.
#![allow(clippy::disallowed_types)]

pub mod tools;

use plugin_toolkit::reqwest;
use plugin_toolkit::secrets::Secret;
use plugin_toolkit::serde_json;
use plugin_toolkit::service::{
    BoxFuture, Routes, Runtime, ServiceBackend, ServiceCapability, ServiceError, ServiceStatus,
    WorkloadSpec,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// unifi service backend — UniFi Network Controller.
///
/// Implements `ServiceBackend` so the generic `service.*` tools
/// (deploy/backup/restore/configure/status/connect/sync) drive unifi. This
/// facet is registered ALONGSIDE the `#[orca_tool]` REST surface in [`tools`] —
/// one binary, both facets, via the `Plugin` builder. Modeled on the nfs
/// StorageBackend. See orca/docs/PLUGIN-PROGRAM.md.
///
/// Holds only the provider name; per-instance routes/creds come from the
/// `Routes` the generic `service.*` tools hand each op.
#[derive(Debug, Clone)]
pub struct UnifiBackend {
    provider: &'static str,
}

impl UnifiBackend {
    pub fn new(provider: &'static str) -> Self {
        Self { provider }
    }
}

impl ServiceBackend for UnifiBackend {
    fn provider(&self) -> &str {
        self.provider
    }

    /// Runtimes unifi can be placed on. `service.deploy` hands the
    /// `workload_spec` below to a matching deploy target — this backend never
    /// drives pct/docker itself (that mechanic lives in the deploy-target domain).
    fn runtimes(&self) -> Vec<Runtime> {
        vec![Runtime::Docker, Runtime::Podman, Runtime::Lxc, Runtime::Vm]
    }

    fn capabilities(&self) -> Vec<ServiceCapability> {
        vec![
            ServiceCapability::Deploy,
            ServiceCapability::Backup,
            ServiceCapability::Restore,
            ServiceCapability::Configure,
            ServiceCapability::Status,
        ]
    }

    fn default_port(&self) -> u16 {
        8443
    }

    /// In-workload paths holding config/data. This is ALL unifi declares for
    /// backup — the generic pluggable backup (tar for containers/LXC, PBS for
    /// Proxmox guests when available) snapshots these. No backup/restore code
    /// here; those are inherited from ServiceBackend's defaults.
    fn data_paths(&self) -> Vec<String> {
        vec!["/config".to_string()]
    }

    fn workload_spec<'a>(
        &'a self,
        _runtime: Runtime,
        _instance: &'a str,
        _routes: &'a Routes,
    ) -> BoxFuture<'a, Result<WorkloadSpec, ServiceError>> {
        // TODO: describe the unifi workload (image/template, ports, mounts,
        // env) for the chosen runtime. The deploy target turns this into a
        // compose service / LXC config / VM. See deploy-target::WorkloadSpec.
        Box::pin(async move { Err(ServiceError::unimplemented("unifi.workload_spec")) })
    }

    fn configure<'a>(
        &'a self,
        _instance: &'a str,
        _routes: &'a Routes,
        _config: &'a str,
    ) -> BoxFuture<'a, Result<(), ServiceError>> {
        // TODO: apply unifi-specific config idempotently.
        Box::pin(async move { Err(ServiceError::unimplemented("unifi.configure")) })
    }

    fn status<'a>(
        &'a self,
        _instance: &'a str,
        _routes: &'a Routes,
    ) -> BoxFuture<'a, Result<ServiceStatus, ServiceError>> {
        // TODO: real health/diagnostics.
        Box::pin(async move { Err(ServiceError::unimplemented("unifi.status")) })
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Wire types
// ═══════════════════════════════════════════════════════════════════════════

/// A configured site on the controller. Mirrors the Integration API's
/// `SiteOverview` — only `id` is guaranteed present.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Site {
    pub id: String,
    #[serde(default)]
    pub name: String,
}

/// A managed device (AP/switch/gateway). Mirrors the Integration API's
/// `DeviceOverview` — `id` and `mac` are required; everything else is
/// best-effort.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Device {
    pub id: String,
    pub mac: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub model: String,
    /// `ONLINE` / `OFFLINE` / `PENDING`, etc.
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub adopted: bool,
    #[serde(default)]
    pub ip: Option<String>,
}

/// A currently connected client (wired or wireless). Mirrors the Integration
/// API's `ClientOverview` — only `id` is required.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Client {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub ip_address: String,
    #[serde(default)]
    pub mac_address: String,
    /// `WIRED` / `WIRELESS`.
    #[serde(default)]
    pub connection_type: String,
    /// Id of the AP/switch this client is connected through.
    #[serde(default)]
    pub access_device_id: Option<String>,
}

/// The Integration API's pagination envelope: `{ "data": [...], ... }`.
#[derive(Debug, Deserialize)]
struct Page<T> {
    #[serde(default = "Vec::new")]
    data: Vec<T>,
}

#[derive(Debug, Error)]
pub enum UnifiError {
    #[error("unifi transport: {0}")]
    Transport(String),
    #[error("unifi api error (status {status}): {body}")]
    Api { status: u16, body: String },
    #[error("malformed unifi response: {0}")]
    Malformed(String),
}

#[derive(Debug, Clone)]
pub struct Config {
    /// Base URL of the UniFi controller (e.g. `https://host:8443`). The
    /// `/proxy/network/integration/v1/<path>` path is joined onto this.
    pub base_url: String,
    /// Local API key, sent as the `X-API-KEY` header.
    pub api_key: Secret<String>,
    /// Skip TLS verification (self-signed controller certs).
    pub insecure: bool,
}

impl Config {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: Secret::new(api_key.into()),
            insecure: false,
        }
    }

    pub fn insecure(mut self, on: bool) -> Self {
        self.insecure = on;
        self
    }

    /// Build the cap-backed HTTP client with the `X-API-KEY` header
    /// pre-attached and TLS verification toggled per `insecure`.
    pub fn build_client(&self) -> Result<reqwest::Client, UnifiError> {
        plugin_toolkit::api_client::ApiClientBuilder::new()
            .header("x-api-key", self.api_key.expose_secret())
            .and_then(|b| b.insecure(self.insecure).build())
            .map_err(|e| UnifiError::Transport(format!("client build: {e}")))
    }

    /// Join an Integration API path onto `/proxy/network/integration/v1`.
    fn api_url(&self, path: &str) -> String {
        format!(
            "{}/proxy/network/integration/v1/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }
}

fn transport(e: impl std::fmt::Display) -> UnifiError {
    UnifiError::Transport(e.to_string())
}

/// `GET <url>` -> deserialize the JSON body into `T`.
async fn get_json<T: DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
) -> Result<T, UnifiError> {
    let resp = client.get(url).send().await.map_err(transport)?;
    let status = resp.status();
    let body = resp.text().await.map_err(transport)?;
    if !status.is_success() {
        return Err(UnifiError::Api {
            status: status.as_u16(),
            body,
        });
    }
    serde_json::from_str(&body).map_err(|e| UnifiError::Malformed(e.to_string()))
}

/// `GET /proxy/network/integration/v1/sites` — every configured site.
pub async fn list_sites(client: &reqwest::Client, cfg: &Config) -> Result<Vec<Site>, UnifiError> {
    let url = cfg.api_url("sites");
    let page: Page<Site> = get_json(client, &url).await?;
    Ok(page.data)
}

/// `GET /proxy/network/integration/v1/sites/<site_id>/devices` — every
/// managed device on the site.
pub async fn list_devices(
    client: &reqwest::Client,
    cfg: &Config,
    site_id: &str,
) -> Result<Vec<Device>, UnifiError> {
    let url = cfg.api_url(&format!("sites/{site_id}/devices"));
    let page: Page<Device> = get_json(client, &url).await?;
    Ok(page.data)
}

/// `GET /proxy/network/integration/v1/sites/<site_id>/clients` — every
/// currently connected client on the site.
pub async fn list_clients(
    client: &reqwest::Client,
    cfg: &Config,
    site_id: &str,
) -> Result<Vec<Client>, UnifiError> {
    let url = cfg.api_url(&format!("sites/{site_id}/clients"));
    let page: Page<Client> = get_json(client, &url).await?;
    Ok(page.data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declares_provider() {
        let b = UnifiBackend::new("unifi");
        assert_eq!(b.provider(), "unifi");
    }

    #[test]
    fn api_url_joins_cleanly() {
        let cfg = Config::new("https://host:8443/", "k");
        assert_eq!(
            cfg.api_url("sites"),
            "https://host:8443/proxy/network/integration/v1/sites"
        );
        assert_eq!(
            cfg.api_url("/sites/default/devices"),
            "https://host:8443/proxy/network/integration/v1/sites/default/devices"
        );
    }

    #[test]
    fn page_envelope_defaults_empty() {
        let empty: Page<Site> = serde_json::from_str("{}").unwrap();
        assert!(empty.data.is_empty());
    }

    /// Run a list fn against a mocked `http.request` capability that answers
    /// each GET by URL suffix; any non-GET or unrouted request fails.
    fn with_mock<T>(
        routes: &[(&str, u16, &str)],
        f: impl FnOnce(
            reqwest::Client,
            Config,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, UnifiError>>>>,
    ) -> Result<T, UnifiError> {
        use plugin_toolkit::abi::{HttpRequest, HttpResponse};
        let routes: Vec<(String, u16, String)> = routes
            .iter()
            .map(|(p, s, b)| (p.to_string(), *s, b.to_string()))
            .collect();
        let sink = Box::new(move |cap: &str, op: &str| {
            assert_eq!(cap, "http.request");
            let req: HttpRequest = serde_json::from_str(op).unwrap();
            assert_eq!(req.method, "GET", "read tool must only GET");
            let (_, status, body) = routes
                .iter()
                .find(|(suffix, _, _)| req.url.ends_with(suffix.as_str()))
                .unwrap_or_else(|| panic!("unexpected request {}", req.url));
            Ok::<String, String>(
                serde_json::to_string(&HttpResponse {
                    status: *status,
                    headers: vec![],
                    body: body.as_bytes().to_vec(),
                })
                .unwrap(),
            )
        });
        let cfg = Config::new("https://10.0.0.1:8443", "k").insecure(true);
        plugin_toolkit::capsink::with_cap_sink(sink, || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            let client = cfg.build_client()?;
            rt.block_on(f(client, cfg))
        })
    }

    #[test]
    fn list_sites_parses_rows() {
        let got = with_mock(
            &[(
                "/sites",
                200,
                r#"{"data":[{"id":"abc123","name":"Default"}]}"#,
            )],
            |c, cfg| Box::pin(async move { list_sites(&c, &cfg).await }),
        )
        .unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].id, "abc123");
        assert_eq!(got[0].name, "Default");
    }

    #[test]
    fn list_sites_rejects_a_row_missing_id() {
        let err = with_mock(
            &[("/sites", 200, r#"{"data":[{"name":"Default"}]}"#)],
            |c, cfg| Box::pin(async move { list_sites(&c, &cfg).await }),
        )
        .unwrap_err();
        assert!(matches!(err, UnifiError::Malformed(_)), "{err}");
    }

    #[test]
    fn list_devices_parses_rows() {
        let got = with_mock(
            &[(
                "/sites/default/devices",
                200,
                r#"{"data":[{"id":"d1","mac":"aa:bb:cc:dd:ee:ff","name":"ap-1","model":"U6-Pro","state":"ONLINE","adopted":true,"ip":"10.0.0.5"}]}"#,
            )],
            |c, cfg| Box::pin(async move { list_devices(&c, &cfg, "default").await }),
        )
        .unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].mac, "aa:bb:cc:dd:ee:ff");
        assert_eq!(got[0].state, "ONLINE");
        assert!(got[0].adopted);
    }

    #[test]
    fn list_devices_rejects_a_row_missing_mac() {
        let err = with_mock(
            &[("/sites/default/devices", 200, r#"{"data":[{"id":"d1"}]}"#)],
            |c, cfg| Box::pin(async move { list_devices(&c, &cfg, "default").await }),
        )
        .unwrap_err();
        assert!(matches!(err, UnifiError::Malformed(_)), "{err}");
    }

    #[test]
    fn list_clients_parses_rows() {
        let got = with_mock(
            &[(
                "/sites/default/clients",
                200,
                r#"{"data":[{"id":"c1","name":"laptop","hostname":"laptop.local","ip_address":"10.0.0.50","mac_address":"11:22:33:44:55:66","connection_type":"WIRELESS","access_device_id":"d1"}]}"#,
            )],
            |c, cfg| Box::pin(async move { list_clients(&c, &cfg, "default").await }),
        )
        .unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].ip_address, "10.0.0.50");
        assert_eq!(got[0].connection_type, "WIRELESS");
        assert_eq!(got[0].access_device_id.as_deref(), Some("d1"));
    }

    #[test]
    fn list_clients_rejects_a_row_missing_id() {
        let err = with_mock(
            &[(
                "/sites/default/clients",
                200,
                r#"{"data":[{"name":"laptop"}]}"#,
            )],
            |c, cfg| Box::pin(async move { list_clients(&c, &cfg, "default").await }),
        )
        .unwrap_err();
        assert!(matches!(err, UnifiError::Malformed(_)), "{err}");
    }

    #[test]
    fn list_sites_surfaces_api_errors() {
        let err = with_mock(&[("/sites", 401, "unauthorized")], |c, cfg| {
            Box::pin(async move { list_sites(&c, &cfg).await })
        })
        .unwrap_err();
        assert!(matches!(err, UnifiError::Api { status: 401, .. }), "{err}");
    }
}
