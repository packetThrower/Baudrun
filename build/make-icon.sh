#!/usr/bin/env bash
# Regenerate build/appicon.png.
# A stylized DB-9 serial connector on a rounded navy square.
# Requires ImageMagick 7 (magick).

set -euo pipefail
cd "$(dirname "$0")"

magick -size 1024x1024 xc:none \
  -fill "#162338" \
  -draw "roundrectangle 0,0 1024,1024 225,225" \
  -fill "#c5ccd6" \
  -draw "polygon 174,307 850,307 782,717 242,717" \
  -fill "#1a1e28" \
  -draw "polygon 199,332 825,332 757,692 267,692" \
  -fill "#e8b46c" \
  -draw "circle 284,447 314,447" \
  -draw "circle 398,447 428,447" \
  -draw "circle 512,447 542,447" \
  -draw "circle 626,447 656,447" \
  -draw "circle 740,447 770,447" \
  -draw "circle 358,591 388,591" \
  -draw "circle 460,591 490,591" \
  -draw "circle 563,591 593,591" \
  -draw "circle 665,591 695,591" \
  appicon.png

echo "Wrote $(pwd)/appicon.png"
