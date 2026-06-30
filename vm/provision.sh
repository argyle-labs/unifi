#!/usr/bin/env bash
# Creates and configures a unifi VM on Proxmox VE. Run on the host as root.
set -euo pipefail
VMID="${1:?Usage: $0 <vmid> [options]}"
# TODO: qm create / cloud-init / install unifi.
echo "[provision] unifi VM $VMID — not yet implemented"
