# UniFi Setup — Connecting orca to a Controller

This plugin does not deploy anything. It connects orca to an existing UniFi
Network controller (self-hosted, UniFi OS console, or a Network Application
running in an LXC/VM). Stand the controller up from the upstream project, then
point orca at it.

---

## Prerequisites

Before configuring the plugin, confirm the controller is reachable:

- **UniFi Network controller** running and reachable at `https://<controller-ip>:8443`
  (UniFi OS consoles typically front this on `https://<controller-ip>` / port 443).
- **Credentials** — a local UniFi account (username/password) or an API token
  the plugin can authenticate with.
- Network path from the orca host to the controller endpoint.

---

## Controller Access

The UniFi Network Application exposes:

- **Web UI:** `https://<controller-ip>:8443`
- **Inform endpoint (for adopted devices):** `http://<controller-ip>:8080/inform`

Verify the controller is healthy and that its managed devices show as
**Connected** under **Devices** before pointing orca at it. If you run the
controller in an LXC/VM, `systemctl status unifi` on that host confirms the
service is up.

---

## Configuring the plugin

orca drives this plugin through its generic service surface. Provide the
controller endpoint and credentials via `service.configure`; the plugin
authenticates against the controller and reports back through the typed
`service.status` payload — never bespoke tools.

Required connection details:

| Field | Value |
|-------|-------|
| Host | `<controller-ip>` (or hostname) |
| Port | `8443` (self-hosted) / `443` (UniFi OS console) |
| Credentials | local account username/password, or API token |

Once configured, `service.status` returns UniFi-specific data (controller
health, adopted devices, SSIDs) inside the typed status payload.

---

## Controller Maintenance

These are operator notes for the controller itself — independent of orca.

### Restart the UniFi service

If the controller runs in an LXC/VM:

```bash
ssh root@<controller-ip>
systemctl restart unifi
systemctl status unifi
```

### Backups

The UniFi controller stores its configuration under
`/var/lib/unifi/backup/` on the controller host. Auto-backup is configured
under **Settings > System > Backup**. Controller config (SSIDs, device
settings) changes rarely and can be exported as `.unf` files.

---

## Troubleshooting

| Problem | Check |
|---------|-------|
| Controller web UI unreachable | Verify the controller host is up; `systemctl status unifi` if self-hosted |
| Device not adopting | Device must reach the controller inform endpoint; try `set-inform http://<controller-ip>:8080/inform` from the device via SSH |
| orca can't authenticate | Confirm credentials/token and that the endpoint (host/port) is correct and reachable from the orca host |
