//! unifi tool surface.
//!
//! Endpoint registry: `unifi.{list, detail, create, update, delete}` —
//! generated wholesale by `#[endpoint_resource]` (row struct, db helpers, schema
//! fragment, args/output types, and the five `#[orca_tool]` fns).
//!
//! Hand-written, read-only tools over the Integration API
//! (`/proxy/network/integration/v1`):
//!   - `unifi.sites`     list configured sites on the controller
//!   - `unifi.devices`   list managed devices on a site (adopted, online/offline, model)
//!   - `unifi.clients`   list currently connected clients on a site (wired/wireless)
//!
//! Imports flow through `plugin_toolkit::prelude::*`.

use plugin_toolkit::prelude::*;

use crate::{Client, Config, Device, Site};

// `routes` is a built-in column on every `#[endpoint_resource]` — an ordered
// fallback list (`--route kind=url`, repeatable, e.g. `--route lan=https://host`)
// resolved by `route::resolve_reachable`. Each entry's free-form `kind`
// (`fqdn` / `lan` / `tailscale`) doubles as the locality class the fewest-hop
// router consumes.
#[endpoint_resource(plugin = "unifi")]
pub struct UnifiEndpoint {
    pub name: String,
    #[secret]
    pub api_key: String,
    pub insecure: bool,
    pub enabled: bool,
}

/// Resolve a registered endpoint into a ready [`Config`]: the first reachable
/// base URL (`resolve_reachable` over the endpoint's `routes` fallback list)
/// plus the local API key from the secrets store.
async fn resolve_config(name: &str) -> Result<Config> {
    let row = endpoint_db::require(name)?;
    let api_key = plugin_toolkit::secrets::resolve_scoped("unifi", name, "api_key")?;
    let base_url = route::resolve_reachable(name, &row.routes, row.insecure).await?;
    Ok(Config {
        base_url,
        api_key,
        insecure: row.insecure,
    })
}

#[derive(clap::Args, Serialize, Deserialize, JsonSchema)]
pub struct EndpointArgs {
    /// Registered unifi endpoint name.
    #[arg(long)]
    pub name: String,
}

/// List every site configured on a UniFi controller.
#[orca_tool(domain = "unifi", verb = "sites", role = "any")]
async fn unifi_sites(args: EndpointArgs, _ctx: &ToolCtx) -> Result<Vec<Site>> {
    let cfg = resolve_config(&args.name).await?;
    let client = cfg.build_client()?;
    Ok(crate::list_sites(&client, &cfg).await?)
}

#[derive(clap::Args, Serialize, Deserialize, JsonSchema)]
pub struct SiteArgs {
    /// Registered unifi endpoint name.
    #[arg(long)]
    pub name: String,
    /// Site id (from `unifi.sites`). Defaults to the controller's default site.
    #[arg(long, default_value = "default")]
    #[serde(default = "default_site_id")]
    pub site_id: String,
}

fn default_site_id() -> String {
    "default".to_string()
}

/// List managed devices (APs/switches/gateways) on a site, with their basic
/// status (adopted, online/offline, model).
#[orca_tool(domain = "unifi", verb = "devices", role = "any")]
async fn unifi_devices(args: SiteArgs, _ctx: &ToolCtx) -> Result<Vec<Device>> {
    let cfg = resolve_config(&args.name).await?;
    let client = cfg.build_client()?;
    Ok(crate::list_devices(&client, &cfg, &args.site_id).await?)
}

/// List currently connected clients (wired/wireless) on a site, with basic
/// info (name/hostname, IP, connected AP/switch).
#[orca_tool(domain = "unifi", verb = "clients", role = "any")]
async fn unifi_clients(args: SiteArgs, _ctx: &ToolCtx) -> Result<Vec<Client>> {
    let cfg = resolve_config(&args.name).await?;
    let client = cfg.build_client()?;
    Ok(crate::list_clients(&client, &cfg, &args.site_id).await?)
}
