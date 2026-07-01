# UniFi WiFi Setup — <your-domain> Homelab

The UniFi controller (LXC at `<ip>` on <host>) manages WiFi access points only. Routing, VLANs, DHCP, firewall, and VPN are all handled by OPNsense (VMID 103). The APs receive tagged VLAN frames from OPNsense via the MikroTik switches and present them as separate SSIDs.

---

## Overview

| Component | What it does |
|-----------|-------------|
| OPNsense VMID 103 | Router, DHCP, firewall, VPN — all networks |
| UniFi controller LXC <ip> | Manages APs — SSIDs, VLAN tagging, radio config |
| MikroTik backbone switch | Trunks VLANs to AP ports (required) |
| UniFi APs (WiFi 6 Lite) | Serve SSIDs, tag traffic to correct VLANs |

The APs do not route anything. They map each SSID to a VLAN tag, and OPNsense handles everything from there.

---

## Current State

| SSID | VLAN | Network | Status |
|------|------|---------|--------|
| <ssid> | untagged (LAN) | <ip>/24 | active |

**Pending decisions:**

- Whether "<ssid>" stays on the primary LAN or moves to become the IoT SSID (VLAN 20), with a new primary SSID created.
- Names for IoT and Guest SSIDs (separate SSIDs or combined with the existing one).

These decisions should be made before proceeding with Part 3 (SSID → VLAN mapping).

---

## Prerequisites

Before configuring additional SSIDs or VLAN-tagged networks on the APs, the following must be in place on OPNsense and the MikroTik switches:

1. **OPNsense:** IoT VLAN interface (vtnet0.20, <ip>/24) and Guest VLAN interface (vtnet0.30, <ip>/24) created and enabled with DHCP. See [opnsense-setup.md → Part 3](opnsense-setup.md#part-3-interface-assignment).

2. **MikroTik backbone switch:** The switch port that each AP is connected to must be configured as a trunk carrying:
   - Untagged: LAN (PVID 1 / native)
   - Tagged: VLAN 20 (IoT)
   - Tagged: VLAN 30 (Guest)

   If an AP port on the MikroTik is currently an untagged access port, it must be changed to a trunk port before IoT/Guest SSIDs will work. See [unifi-setup.md → Part 2](#part-2-mikrotik-switch-trunk-ports-for-aps).

3. **UniFi controller:** running and reachable at `https://<ip>:8443`.

Do not skip step 2 — if the switch port only passes untagged traffic, devices on VLAN-tagged SSIDs will get no DHCP response from OPNsense.

---

## Part 1: Controller Access and AP Status

The UniFi Network Application runs on the controller LXC.

- **Web UI:** `https://<ip>:8443`
- **SSH to LXC:** `ssh root@<ip>`

Verify all APs show as **Connected** under **Devices** before making configuration changes. If an AP shows as disconnected, check:
- Is the AP's switch port up?
- Can the AP reach `<ip>`? (ping from a LAN host; the AP itself is on the LAN)
- `systemctl status unifi` on the LXC

---

## Part 2: MikroTik Switch Trunk Ports for APs

Each AP must be connected to a trunk port on the MikroTik that carries VLANs 20 and 30 tagged. This is done via RouterOS, not the UniFi controller.

**Connect to the backbone switch** via WinBox or WebFig at its management IP.

Identify the port each AP is plugged into, then configure it as a VLAN trunk:

```
# Allow tagged VLAN 20 and 30 through the AP port
/interface bridge vlan
add bridge=bridge1 tagged=<ap-port> vlan-ids=20
add bridge=bridge1 tagged=<ap-port> vlan-ids=30

# Ensure the port passes untagged LAN traffic (PVID 1)
/interface bridge port
set [find interface=<ap-port>] pvid=1 frame-types=admit-all
```

Replace `<ap-port>` with the actual interface name (e.g., `ether5`).

> Do this change **before** adding VLAN-tagged SSIDs in the controller. The order matters — if you add the SSID first, devices will associate but get no IP because tagged frames are blocked at the switch.

After making the change, verify on the switch:
```
/interface bridge vlan print
```
Confirm VLAN 20 and 30 both show the AP port as tagged.

---

## Part 3: WiFi Networks in the UniFi Controller

In the UniFi controller, each WiFi network is defined under **Settings > WiFi**. Each WiFi network is tied to a UniFi **Network** object, which carries the VLAN tag information.

### Step 1: Create Network objects for each VLAN

Go to **Settings > Networks** and confirm these networks exist (they should mirror OPNsense's VLANs):

| Name | VLAN ID | Purpose |
|------|---------|---------|
| LAN | none (untagged) | Primary — <ip>/24 |
| IoT | 20 | IoT — <ip>/24 |
| Guest | 30 | Guest — <ip>/24 |

If IoT or Guest network objects don't exist yet, create them:
- **Settings > Networks > Add Network**
- Set VLAN ID to 20 (IoT) or 30 (Guest)
- **Do not configure DHCP here** — OPNsense handles all DHCP. Leave DHCP off or set to "None".
- Purpose: Corporate (for IoT); Guest (for Guest network)

### Step 2: Create SSIDs

Go to **Settings > WiFi > Add WiFi Network**.

**Primary SSID (LAN)**

| Field | Value |
|-------|-------|
| Name (SSID) | <ssid> *(or new name — TBD)* |
| Network | LAN |
| Security | WPA2/WPA3 |
| Password | (your password) |

**IoT SSID** *(create when ready — after MikroTik trunk ports confirmed)*

| Field | Value |
|-------|-------|
| Name (SSID) | (TBD) |
| Network | IoT |
| Security | WPA2 |
| Password | separate password |

**Guest SSID** *(create when ready)*

| Field | Value |
|-------|-------|
| Name (SSID) | (TBD) |
| Network | Guest |
| Security | WPA2 or Open |
| Password | (optional) |

When you save a WiFi network, the controller pushes the config to all adopted APs. Each AP will begin broadcasting that SSID and tagging client traffic with the associated VLAN.

### Step 3: Verify

After saving, from a device connected to the IoT SSID:
```bash
# Should get an IP in <ip>–200 from OPNsense DHCP
ip addr  # or check device network settings

# Should have internet access
curl -s https://ipinfo.io
```

From a device connected to the Guest SSID:
```bash
# Should get an IP in <ip>–200
# Should have internet access but not reach <subnet>
ping <ip>   # should fail (OPNsense firewall blocks guest → LAN)
```

---

## Part 4: AP Radio Settings

These settings apply globally across all APs via the UniFi controller.

**Settings > WiFi > [SSID] > Advanced** or **Settings > System > Radio Manager**

Recommended settings:
- **Band steering:** enabled (guides dual-band clients to 5 GHz)
- **Minimum RSSI:** -85 dBm (kicks weak clients that hold the AP)
- **BSS Transition:** enabled (802.11v — helps roaming)
- **Fast Roaming:** enabled if all APs support it (802.11r)

No changes needed here for VLAN functionality — these are quality-of-life settings.

---

## Part 5: Controller Maintenance

### Restart UniFi service on the LXC

```bash
ssh root@<ip>
systemctl restart unifi
systemctl status unifi
```

### Update controller

```bash
apt update && apt upgrade -y
```

After upgrading, verify all APs show Connected in the Devices view.

### Backup

The UniFi controller stores its configuration on the LXC at `/var/lib/unifi/backup/`. Auto-backup is configured under **Settings > System > Backup**.

For NFS backup to <host>, the controller LXC can mount `<ip>:/mnt/user/backups` and copy `.unf` files nightly — but this is lower priority than OPNsense backups since the controller config (SSIDs, AP settings) changes rarely and can be re-entered in under an hour if lost.

---

## Troubleshooting

| Problem | Check |
|---------|-------|
| IoT/Guest SSID visible but no IP assigned | MikroTik trunk port not passing tagged VLANs — see Part 2; also verify OPNsense VLAN interfaces and DHCP are enabled |
| AP not adopting | AP must reach controller at `<ip>:8080`; try `set-inform http://<ip>:8080/inform` from the AP via SSH |
| SSID not appearing on APs after save | Controller may not have pushed config; check AP status in Devices view; force re-provision via Devices > [AP] > Config > Force Provision |
| Client on IoT SSID can reach LAN | OPNsense firewall rule missing or wrong — see [opnsense-setup.md → Part 10](opnsense-setup.md#part-10-firewall-rules) |
| Controller web UI unreachable | SSH to LXC: `systemctl status unifi`; check LXC is running in Proxmox |
