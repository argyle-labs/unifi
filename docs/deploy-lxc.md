# UniFi Network Application on a Proxmox LXC (native)

A standalone deployment: the UniFi Network Application (controller) running
**natively** inside an **unprivileged Debian LXC** on Proxmox, managing UniFi
APs, switches, and gateways. Nothing here needs orca.

> Placeholders: `<proxmox-host>` = your Proxmox node, `<ip>` = a LAN address,
> `<pool>` = your ZFS/backup pool. Pick the CT ID with
> `pvesh get /cluster/nextid` (shown as `<CTID>`).

- **Ports**: 8443 (web UI, https), 8080 (device inform/adoption), 3478/udp
  (STUN), 8843/8880 (guest portal), 6789 (throughput test), 10001/udp (discovery)
- **Type**: Proxmox LXC — Debian minimal, **unprivileged**
- **Footprint**: 2 cores / 3 GB RAM / 8 GB disk

The 3 GB RAM covers the bundled MongoDB plus the Java controller — do not
shrink below it. A **static IP** is recommended so adopted devices keep a stable
inform address.

---

## Step 1 — Create the LXC

```bash
pveam available | grep debian-12
pct create "$(pvesh get /cluster/nextid)" \
  local:vztmpl/debian-12-standard_12.7-1_amd64.tar.zst \
  --hostname unifi \
  --storage local-lvm \
  --rootfs local-lvm:8 \
  --cores 2 --memory 3072 --swap 512 \
  --net0 name=eth0,bridge=vmbr0,ip=10.0.0.18/24,gw=10.0.0.1 \
  --features nesting=1,keyctl=1 \
  --unprivileged 1 \
  --onboot 1
```

A full sample config is in [`lxc/unifi.conf.example`](../lxc/unifi.conf.example)
— it also includes a `mp0` backup bind mount. Copy the fields you want into
`/etc/pve/lxc/<CTID>.conf` on `<proxmox-host>` (the CT must be stopped to edit).

## Step 2 — Install the UniFi Network Application

```bash
pct start <CTID>
pct enter <CTID>

apt-get update && apt-get upgrade -y
# use the community-scripts UniFi installer, or Ubiquiti's APT repo, which
# pulls the correct MongoDB + Java (OpenJDK 17) dependencies for the release.
```

Once installed, the `unifi` service starts MongoDB and the controller.

## Step 3 — First-run setup

Open **https://<ip>:8443** (accept the self-signed cert), create the admin
account, and either restore a backup (Step 5) or set up a new site. Devices are
adopted over port **8080** (inform URL `http://<ip>:8080/inform`).

## Step 4 — Adoption

Set the inform host so devices find the controller:

- On a UniFi gateway/network: set the controller/inform host to `<ip>`.
- Manually: SSH to a device and run `set-inform http://<ip>:8080/inform`.

If devices are on a different VLAN, make sure ports 8080 and 3478/udp are
reachable across the boundary.

## Step 5 — Backups

UniFi has a first-class backup. In the UI: **Settings → System → Backup →
Download Backup** (or schedule auto-backups). Auto-backups land in
`/var/lib/unifi/backup/`; mirror them to the `/mnt/backups` bind mount:

```bash
cat > /usr/local/bin/backup-unifi.sh << 'EOF'
#!/bin/sh
set -e
cp -a /var/lib/unifi/backup/autobackup/. /mnt/backups/ 2>/dev/null || true
EOF
chmod +x /usr/local/bin/backup-unifi.sh
```

Schedule with a systemd timer (`OnCalendar=*-*-* 05:30:00`, `Persistent=true`).

**Restore:** on a fresh controller, choose *Restore from backup* at first-run
and upload a `.unf` file.

## Troubleshooting

**Devices won't adopt / stuck "Adopting"** — the inform host is wrong or blocked.
Confirm 8080 is reachable from the device's subnet and the inform URL points at
`<ip>`.

**Controller won't start** — almost always MongoDB. Check
`journalctl -u unifi -n 50`; a corrupt Mongo db or an OOM (raise memory) are the
usual causes.

**Java/Mongo version errors after an OS upgrade** — the UniFi release pins
specific OpenJDK and MongoDB majors; check Ubiquiti's release notes before
upgrading either.
