<p align="center">
  <img src="assets/icon-256.png" width="120" alt="unifi" />
</p>

# unifi

The UniFi Network controller manages Ubiquiti UniFi access points, switches, and gateways.

A first-party [orca](https://github.com/argyle-labs/orca) plugin (appliance integration).

This plugin **connects orca to an existing unifi install** — there's nothing to deploy here. Stand up unifi from the upstream project, then point orca at it.

---

## Run it without orca

Install unifi per the upstream project: <https://ui.com/>. It listens on port `8443` by default; this plugin talks to that endpoint (host, credentials/token) — no container is deployed.


See [unifi-setup.md](docs/unifi-setup.md) for worked operator notes.

## With orca

orca drives this plugin through its generic surface — rich, unifi-specific data comes back in the typed `service.status` payload, never bespoke tools.

## Layout

- `src/` — the plugin (pure Rust): the `ServiceBackend` descriptor + `configure` / `status`.
- `docs/` — standalone operator notes.
- [CAPABILITIES.md](CAPABILITIES.md) — the service-backend contract checklist.
- `assets/` — plugin icon.
