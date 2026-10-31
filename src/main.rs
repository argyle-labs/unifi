//! Dynamic (subprocess) entrypoint for the unifi plugin.
//!
//! A DUAL-facet plugin: one `Plugin` builder chain registers BOTH the
//! [`UnifiBackend`](unifi::UnifiBackend) (generic `service.*` lifecycle) AND
//! the `unifi.` `#[orca_tool]` surface (sites/devices/clients). The plugin is
//! a `[[bin]]`, owns no runtime, and reaches orca only through the socket.
plugin_toolkit::instrument::bootstrap!();

use plugin_toolkit::plugin::Plugin;
use unifi::UnifiBackend;

// Force-link this plugin's OWN lib crate so the linker doesn't dead-strip the
// rlib (and with it every `#[orca_tool]` / `#[endpoint_resource]` registration).
// The builder does NOT force-link for you, so this `use ... as _;` is required.
#[allow(unused_imports)]
use unifi::tools as _;

fn main() -> plugin_toolkit::anyhow::Result<()> {
    Plugin::named("unifi")
        .version(env!("CARGO_PKG_VERSION"))
        .service(UnifiBackend::new("unifi"))
        .tools(["unifi."])
        .serve()
}
