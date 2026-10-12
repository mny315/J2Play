#!/bin/sh
# User-local, versioned installation. Game data is owned by the application.
set -eu
J2PLAY_SOURCE=$(CDPATH= cd -- "$(dirname -- "$(realpath -- "$0")")" && pwd -P)
J2PLAY_DATA=${XDG_DATA_HOME:-${HOME:?HOME is required}/.local/share}
J2PLAY_INSTALL=${1:-$J2PLAY_DATA/j2play-program}
if [ "$#" -gt 1 ]; then
    echo "Usage: install.sh [absolute-program-directory]" >&2
    exit 2
fi
case "$J2PLAY_INSTALL:$J2PLAY_DATA" in
    *'
'*|*''*) echo "Installation paths cannot contain line breaks." >&2; exit 2 ;;
esac
case "$J2PLAY_INSTALL" in /*) ;; *) echo "Use an absolute installation path." >&2; exit 2 ;; esac
case "$J2PLAY_DATA" in /*) ;; *) echo "XDG_DATA_HOME must be absolute." >&2; exit 2 ;; esac
if [ -e "$J2PLAY_INSTALL" ] && [ ! -f "$J2PLAY_INSTALL/.j2play-program" ]; then
    echo "Destination exists and is not a J2Play program installation." >&2
    exit 1
fi
cd "$J2PLAY_SOURCE"
sha256sum -c SHA256SUMS >/dev/null
J2PLAY_VERSION=$(cat build-id)
case "$J2PLAY_VERSION" in ''|*[!0-9a-f]*) echo "Invalid build ID." >&2; exit 1 ;; esac
mkdir -p "$J2PLAY_INSTALL/releases" "$J2PLAY_DATA/applications"
touch "$J2PLAY_INSTALL/.j2play-program"
J2PLAY_STAGE="$J2PLAY_INSTALL/releases/.install-$$"
mkdir "$J2PLAY_STAGE"
trap 'rm -rf -- "$J2PLAY_STAGE"' EXIT HUP INT TERM
cp -R "$J2PLAY_SOURCE/." "$J2PLAY_STAGE/"
(cd "$J2PLAY_STAGE" && sha256sum -c SHA256SUMS >/dev/null)
if [ ! -e "$J2PLAY_INSTALL/releases/$J2PLAY_VERSION" ]; then
    mv "$J2PLAY_STAGE" "$J2PLAY_INSTALL/releases/$J2PLAY_VERSION"
else
    (cd "$J2PLAY_INSTALL/releases/$J2PLAY_VERSION" && sha256sum -c SHA256SUMS >/dev/null)
fi
# Desktop Exec has both desktop-value and shell-like argument escaping.
# Percent is escaped separately because desktop launchers expand field codes.
J2PLAY_EXEC=$(printf '%s' "$J2PLAY_INSTALL/current/J2Play" | awk '
{ for(i=1;i<=length($0);i++) { c=substr($0,i,1);
  if(c=="%") printf "%%%%";
  else if(c=="\\") printf "\\\\\\\\";
  else if(c=="\"" || c=="$" || c=="`") printf "\\\\%s",c;
  else printf "%s",c; } }')
J2PLAY_ICON=$(printf '%s' "$J2PLAY_INSTALL/current/usr/share/icons/hicolor/192x192/apps/io.github.mny315.j2play.png" | sed 's/\\/\\\\/g')
J2PLAY_DESKTOP="$J2PLAY_DATA/applications/.j2play-$$.desktop"
{
    cat "$J2PLAY_SOURCE/usr/share/applications/io.github.mny315.j2play.desktop" | sed '/^Exec=/d; /^Icon=/d'
    # GIO checks the executable before expanding %% in the Exec value. Keep
    # the interpreter's path literal and pass J2Play as one quoted argument.
    printf 'Exec=/bin/sh "%s" %%f\nIcon=%s\n' "$J2PLAY_EXEC" "$J2PLAY_ICON"
} > "$J2PLAY_DESKTOP"
ln -s "releases/$J2PLAY_VERSION" "$J2PLAY_INSTALL/.current-$$"
mv -Tf "$J2PLAY_INSTALL/.current-$$" "$J2PLAY_INSTALL/current"
mv -f "$J2PLAY_DESKTOP" "$J2PLAY_DATA/applications/io.github.mny315.j2play.desktop"
mkdir -p "$J2PLAY_DATA/mime/packages"
cp "$J2PLAY_SOURCE/usr/share/mime/packages/io.github.mny315.j2play.xml" "$J2PLAY_DATA/mime/packages/.j2play-$$.xml"
mv -f "$J2PLAY_DATA/mime/packages/.j2play-$$.xml" "$J2PLAY_DATA/mime/packages/io.github.mny315.j2play.xml"
if command -v update-mime-database >/dev/null 2>&1; then
    update-mime-database "$J2PLAY_DATA/mime" || true
fi
if ! command -v update-desktop-database >/dev/null 2>&1 || ! update-desktop-database "$J2PLAY_DATA/applications"; then
    # Minimal desktops may omit desktop-file-utils. Merge this application's
    # registrations into the cache without changing mimeapps.list preferences.
    J2PLAY_MIME_CACHE="$J2PLAY_DATA/applications/mimeinfo.cache"
    J2PLAY_MIME_INPUT="$J2PLAY_MIME_CACHE"
    [ -f "$J2PLAY_MIME_INPUT" ] || J2PLAY_MIME_INPUT=/dev/null
    J2PLAY_MIME_TYPES=$(sed -n 's/^MimeType=//p' "$J2PLAY_DATA/applications/io.github.mny315.j2play.desktop")
    awk -v types="$J2PLAY_MIME_TYPES" '
        function missing( type) {
            for (type in wanted) {
                print type "=io.github.mny315.j2play.desktop;"
                delete wanted[type]
            }
        }
        BEGIN { count=split(types, names, ";"); for(i=1;i<=count;i++) if(names[i]!="") wanted[names[i]]=1 }
        /^\[/ {
            if (inside) missing()
            inside=($0=="[MIME Cache]"); if(inside) found=1
        }
        inside && /=/ {
            separator=index($0,"="); type=substr($0,1,separator-1)
            if(type in wanted) {
                values=substr($0,separator+1)
                if(index(";" values, ";io.github.mny315.j2play.desktop;")==0)
                    $0=$0 (values!="" && substr(values,length(values),1)!=";" ? ";" : "") "io.github.mny315.j2play.desktop;"
                delete wanted[type]
            }
        }
        { print }
        END { if(!found) print "[MIME Cache]"; missing() }
    ' "$J2PLAY_MIME_INPUT" > "$J2PLAY_DATA/applications/.mimeinfo-$$"
    mv -f "$J2PLAY_DATA/applications/.mimeinfo-$$" "$J2PLAY_MIME_CACHE"
fi
printf 'Installed J2Play: %s/current/J2Play\n' "$J2PLAY_INSTALL"
