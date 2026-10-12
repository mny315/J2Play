#!/bin/bash
# Local Ports prototype. The firmware must provide the documented desktop services.
set -e
J2PLAY_PORTS=$(CDPATH= cd -- "$(dirname -- "$(realpath -- "$0")")" && pwd -P)
J2PLAY_PORT="$J2PLAY_PORTS/j2play"
if [ -d /opt/system/Tools/PortMaster ]; then
    controlfolder=/opt/system/Tools/PortMaster
elif [ -d /opt/tools/PortMaster ]; then
    controlfolder=/opt/tools/PortMaster
elif [ -d "${XDG_DATA_HOME:-${HOME:-/storage}/.local/share}/PortMaster" ]; then
    controlfolder="${XDG_DATA_HOME:-${HOME:-/storage}/.local/share}/PortMaster"
else
    controlfolder="$J2PLAY_PORTS/PortMaster"
fi
if [ ! -f "$controlfolder/control.txt" ]; then
    echo "J2Play needs PortMaster controller configuration; see the included README." >&2
    exit 1
fi
source "$controlfolder/control.txt"
if [ -f "$controlfolder/mod_${CFW_NAME}.txt" ]; then
    source "$controlfolder/mod_${CFW_NAME}.txt"
fi
get_controls
if [ -n "${sdl_controllerconfig:-}" ]; then
    export SDL_GAMECONTROLLERCONFIG="$sdl_controllerconfig"
fi
if [ -z "${WAYLAND_DISPLAY:-}${DISPLAY:-}" ]; then
    pm_message "J2Play requires a Wayland/X11 session, compatible graphics drivers and logind. See the included compatibility matrix."
    exit 1
fi
export XDG_DATA_HOME="$J2PLAY_PORT/userdata"
export XDG_CACHE_HOME="$J2PLAY_PORT/cache"
mkdir -p "$XDG_DATA_HOME" "$XDG_CACHE_HOME"
exec "$J2PLAY_PORT/versions/@BUILD_ID@/J2Play"
