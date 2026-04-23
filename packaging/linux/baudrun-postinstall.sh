#!/bin/sh
# Post-install hook for Baudrun's .deb / .rpm / pacman packages.
# Reloads udev rules and re-triggers the currently-attached devices
# so the newly-installed 60-baudrun-serial.rules (see that file for
# what it does) applies immediately — users don't have to re-plug
# their serial adapter or log out after install.
#
# Wrapped in a check so chroot / container installs (no /run/udev)
# don't fail noisily; udev will pick the rules up next time it
# does start. Missing or non-executable udevadm is treated the same
# way — not every minimal install has it and it's not an error
# worth aborting the install over.

set -e

if command -v udevadm >/dev/null 2>&1 && [ -d /run/udev ]; then
    udevadm control --reload-rules || :
    udevadm trigger --subsystem-match=tty --subsystem-match=usb --action=change || :
fi

exit 0
