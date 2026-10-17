#!/bin/sh
# Install the secbird CLI and the udev rule that lets you use Razer devices without root.
#   sudo ./install.sh              install to /usr/local/bin
#   sudo ./install.sh --uninstall  remove both
set -e
cd "$(dirname "$0")"
bin=/usr/local/bin/secbird
rule=/etc/udev/rules.d/70-secretarybird.rules
if [ "$(id -u)" -ne 0 ]; then
    echo "Run it with sudo: sudo $0 $*" >&2
    exit 1
fi
if [ "$1" = "--uninstall" ]; then
    rm -f "$bin" "$rule"
else
    install -m 755 secbird "$bin"
    install -m 644 70-secretarybird.rules "$rule"
fi
udevadm control --reload-rules 2>/dev/null || true
udevadm trigger --subsystem-match=hidraw 2>/dev/null || true
if [ "$1" = "--uninstall" ]; then
    echo "Removed $bin and $rule. Your settings stay in ~/.config/secretarybird."
else
    echo "Installed $bin and $rule. Try: secbird list"
    echo "If your device isn't listed, unplug it and plug it back in."
fi
