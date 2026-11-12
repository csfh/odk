#!/bin/sh
# Installs odk for the user who runs it (ADR 0063, ADR 0086): the `odk` command, the programs
# behind it, and the `odkd` daemon with its systemd user units, and the `.oma` file type
# (ADR 0065) with its handler (ADR 0097). No root, nothing outside the home directory.
#
#   curl --proto '=https' --tlsv1.2 -fsS -o install.sh https://odk.csfh.dev/install.sh
#   curl --proto '=https' --tlsv1.2 -fsS -o install.sh.sig https://odk.csfh.dev/install.sh.sig
#
# Check install.sh.sig against the odk release root, as packaging/README.md says, and only then:
#
#   sh install.sh
#
# The signature covers the tarball too: this script names it by length and SHA-256.
#
# Options:  --uninstall     remove what this script installed (application data is kept)
#           --help
#
# Where things go:  ~/.local/bin/odk
#                   ~/.local/lib/odk/{odk-oma,oma-consent,odkd}
#                   ${XDG_CONFIG_HOME:-~/.config}/systemd/user/{odkd.socket,odkd-files.socket,odkd.service,
#                       oma-register.service,oma-applications.path,oma-inbox@.path}
#                   ${XDG_DATA_HOME:-~/.local/share}/mime/packages/oma.xml
#                   ${XDG_DATA_HOME:-~/.local/share}/applications/odk-oma.desktop, which runs a
#                       .oma file, and is made the default for the type in
#                       ${XDG_CONFIG_HOME:-~/.config}/mimeapps.list (ADR 0097)
#                   ${XDG_DATA_HOME:-~/.local/share}/doc/odk/{LICENSE,THIRD-PARTY-LICENSES}
#                   ${XDG_DATA_HOME:-~/.local/share}/oma/runtime.toml, the release installed
#
# It is never run as root (ADR 0087). Running it again upgrades; a release older than the one
# installed is refused. `ODK_BASE_URL` names another place to download from, for tests.
set -eu

# --- the release --------------------------------------------------------------------------
# `tools/package-release` writes these when it makes a release, and the release root's signature
# over this script covers them (ADR 0086). In the repository they are empty.
release_version=
tarball_sha256=
tarball_length=

action=install

say() { printf 'odk install: %s\n' "$*"; }
die() { printf 'odk install: %s\n' "$*" >&2; exit 1; }

while [ $# -gt 0 ]; do
    case $1 in
        --uninstall) action=uninstall ;;
        --help | -h) sed -n '2,27p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) die "unknown option '$1' (try --help)" ;;
    esac
    shift
done

# --- where --------------------------------------------------------------------------------
[ "$(id -u)" -ne 0 ] \
    || die "run this as your own user, not as root: odk installs under your home directory only (ADR 0087). Nothing was written"
case ${HOME:-} in /*) ;; *) die "HOME is not an absolute path" ;; esac
prefix=$HOME/.local
bin=$prefix/bin
lib=$prefix/lib/odk
case ${XDG_CONFIG_HOME:-} in /*) config=$XDG_CONFIG_HOME ;; *) config=$HOME/.config ;; esac
units=$config/systemd/user
case ${XDG_DATA_HOME:-} in /*) data=$XDG_DATA_HOME ;; *) data=$HOME/.local/share ;; esac
mime=$data/mime
apps=$data/applications
receipt=$data/oma/runtime.toml
default_line='application/x-oma=odk-oma.desktop'

# The file type is known to the desktop once its database is rebuilt. Without the tool the
# type is in place for when it is there.
refresh_types() {
    if command -v update-mime-database >/dev/null 2>&1; then
        update-mime-database "$mime" || say "note: update-mime-database failed; .oma files may not be recognised yet"
    else
        say "note: update-mime-database (shared-mime-info) is not installed, so .oma files are not recognised yet"
    fi
}

# The desktop's list of handlers is rebuilt when the tool is there. A handler is found by its
# file all the same, so a missing tool is a note and not a failure.
refresh_apps() {
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$apps" || say "note: update-desktop-database failed; the handler may not be listed yet"
    else
        say "note: update-desktop-database (desktop-file-utils) is not installed; the handler is found by its file"
    fi
}

# Makes the handler the default for a .oma file: the line is put first under [Default Applications]
# in mimeapps.list, in place of any other default for the type, and the rest of the file is kept.
set_default() {
    list=$config/mimeapps.list
    mkdir -p "$config"
    if [ -f "$list" ]; then
        awk -v line="$default_line" '
            /^\[.*\]$/ { sect = $0 }
            sect == "[Default Applications]" && index($0, "application/x-oma=") == 1 { next }
            { print }
            $0 == "[Default Applications]" { print line; done = 1 }
            END { if (!done) printf "[Default Applications]\n%s\n", line }
        ' "$list" >"$list.new.$$"
    else
        printf '[Default Applications]\n%s\n' "$default_line" >"$list.new.$$"
    fi
    mv -f "$list.new.$$" "$list"
    say "made $apps/odk-oma.desktop the default for .oma files in $list"
}

# Takes the line out again if it is the one this script wrote, and the file with it when nothing
# else is left in it.
drop_default() {
    list=$config/mimeapps.list
    [ -f "$list" ] || return 0
    grep -qxF "$default_line" "$list" || return 0
    grep -vxF "$default_line" "$list" >"$list.new.$$" || true
    if grep -qv '^\[.*\]$\|^[[:space:]]*$' "$list.new.$$"; then
        mv -f "$list.new.$$" "$list"
    else
        rm -f "$list.new.$$" "$list"
    fi
    say "removed the default for .oma files from $list"
}

# A user manager is there when it answers. Over ssh with no session bus it does not.
have_user_manager() {
    command -v systemctl >/dev/null 2>&1 && systemctl --user show-environment >/dev/null 2>&1
}

path_note() {
    case :${PATH:-}: in
        *":$bin:"*) ;;
        *) say "note: $bin is not on your PATH; add it to run \`odk\`" ;;
    esac
}

# --- uninstall ----------------------------------------------------------------------------
if [ "$action" = uninstall ]; then
    if have_user_manager; then
        systemctl --user disable --now odkd.socket odkd-files.socket odkd.service >/dev/null 2>&1 || true
        systemctl --user disable --now oma-register.service oma-applications.path >/dev/null 2>&1 || true
        # The inbox watches `odk oma --register` enabled, one per application (ADR 0015).
        for watch in "$units"/default.target.wants/oma-inbox@*.path; do
            [ -L "$watch" ] || continue
            systemctl --user disable --now "$(basename "$watch")" >/dev/null 2>&1 || true
        done
    fi
    for file in "$bin/odk" "$lib/odk-oma" "$lib/oma-consent" "$lib/odkd" \
        "$units/odkd.socket" "$units/odkd-files.socket" "$units/odkd.service" \
        "$units/oma-register.service" "$units/oma-applications.path" "$units/oma-inbox@.path" \
        "$mime/packages/oma.xml" "$apps/odk-oma.desktop" \
        "$data/doc/odk/LICENSE" "$data/doc/odk/THIRD-PARTY-LICENSES"; do
        if [ -e "$file" ]; then
            rm -f "$file"
            say "removed $file"
        fi
    done
    rmdir "$lib" "$data/doc/odk" 2>/dev/null || true
    if have_user_manager; then systemctl --user daemon-reload || true; fi
    if [ -d "$mime/packages" ]; then refresh_types; fi
    drop_default
    if [ -d "$apps" ]; then refresh_apps; fi
    say "done; application data under ~/.local/share/oma, and the record of the release installed, are kept"
    exit 0
fi

# --- checks -------------------------------------------------------------------------------
# The handler's Exec names the `odk` installed, quoted; a path with a character that a .desktop
# file or the substitution below would read as something else is refused before anything is written.
case $bin in
    *[\"\\\`\$\|\&]*) die "$bin has a character a handler cannot carry (one of \" \\ \` \$ | &); nothing was installed" ;;
esac
[ "$(uname -s)" = Linux ] || die "odk runs on Linux only"
[ "$(uname -m)" = x86_64 ] || die "odk is built for x86_64 only"
for tool in curl tar gzip sha256sum mktemp install; do
    command -v "$tool" >/dev/null 2>&1 || die "needs '$tool' and did not find it"
done

# --- download -----------------------------------------------------------------------------
[ -n "$release_version" ] && [ -n "$tarball_sha256" ] && [ -n "$tarball_length" ] \
    || die "this install.sh names no release; download the one at https://odk.csfh.dev/install.sh"
# An older release than the one installed is refused: a signed script stays valid, and must not
# take an install back (ADR 0086). The same release again is a repair.
if [ -f "$receipt" ]; then
    installed=$(sed -n 's/^version = \([0-9][0-9]*\)$/\1/p' "$receipt")
    [ -n "$installed" ] || die "$receipt names no version; nothing was installed"
    [ "$release_version" -ge "$installed" ] \
        || die "release $release_version is older than the installed release $installed; nothing was installed"
fi
name=odk-x86_64-linux
base=${ODK_BASE_URL:-https://odk.csfh.dev}
# Only HTTPS, or a local file for tests, and no redirect is followed (ADR 0060, section 8).
case $base in
    https://*) fetch="curl --proto =https --tlsv1.2 -fsS" ;;
    file://*) fetch="curl --proto =file -fsS" ;;
    *) die "the download address must be https:// (or file:// for a test): $base" ;;
esac

work=$(mktemp -d "${TMPDIR:-/tmp}/odk-install.XXXXXX")
chmod 700 "$work"
trap 'rm -rf "$work"' EXIT HUP INT TERM

say "downloading $base/$name.tar.gz"
$fetch -o "$work/$name.tar.gz" "$base/$name.tar.gz" || die "cannot download $base/$name.tar.gz"

# The tarball must be the one this script names. The release root's signature over the script
# is what says who published it (ADR 0086).
have_length=$(wc -c <"$work/$name.tar.gz" | tr -d ' ')
have=$(sha256sum "$work/$name.tar.gz" | cut -d' ' -f1)
[ "$have_length" = "$tarball_length" ] && [ "$have" = "$tarball_sha256" ] \
    || die "the download is not the tarball this script names; nothing was installed"
say "release $release_version, tarball ok"

# --- unpack, and look before touching anything ------------------------------------------------
tar -tzf "$work/$name.tar.gz" >"$work/entries" || die "the download is not a tarball; nothing was installed"
# Read from a file, not a pipe: a `die` in a pipeline would only end the pipeline.
while IFS= read -r entry; do
    case $entry in
        odk | odk/ | odk/*) ;;
        *) die "the tarball holds '$entry', which is outside odk/; nothing was installed" ;;
    esac
    case /$entry/ in
        */../*) die "the tarball holds '$entry'; nothing was installed" ;;
    esac
done <"$work/entries"
mkdir "$work/stage"
tar -xzf "$work/$name.tar.gz" -C "$work/stage" --no-same-owner --no-same-permissions
src=$work/stage/odk
for file in VERSION bin/odk lib/odk/odk-oma lib/odk/oma-consent lib/odk/odkd \
    share/systemd/user/odkd.socket share/systemd/user/odkd.service share/mime/packages/oma.xml; do
    [ -f "$src/$file" ] && [ ! -L "$src/$file" ] || die "the tarball has no $file; nothing was installed"
done
version=$(cat "$src/VERSION")

# --- install ------------------------------------------------------------------------------
# Each file is written under a temporary name in its own directory and renamed into place, so
# a running odkd or a reader never sees half a file.
put() { # mode source destination
    mkdir -p "$(dirname "$3")"
    install -m "$1" "$2" "$3.new.$$"
    mv -f "$3.new.$$" "$3"
    say "wrote $3"
}

put 755 "$src/bin/odk" "$bin/odk"
put 755 "$src/lib/odk/odk-oma" "$lib/odk-oma"
put 755 "$src/lib/odk/oma-consent" "$lib/oma-consent"
put 755 "$src/lib/odk/odkd" "$lib/odkd"
put 644 "$src/share/systemd/user/odkd.socket" "$units/odkd.socket"
# The file chooser's socket (ADR 0068); a release before it has none, and odkd makes its own.
sockets=odkd.socket
if [ -f "$src/share/systemd/user/odkd-files.socket" ] && [ ! -L "$src/share/systemd/user/odkd-files.socket" ]; then
    put 644 "$src/share/systemd/user/odkd-files.socket" "$units/odkd-files.socket"
    sockets="odkd.socket odkd-files.socket"
fi
put 644 "$src/share/systemd/user/odkd.service" "$units/odkd.service"
# What runs `odk oma --register` (ADR 0009, ADR 0053); a release before it has none.
registration=
if [ -f "$src/share/systemd/user/oma-register.service" ] && [ ! -L "$src/share/systemd/user/oma-register.service" ]; then
    for unit in oma-register.service oma-applications.path oma-inbox@.path; do
        put 644 "$src/share/systemd/user/$unit" "$units/$unit"
    done
    registration="oma-register.service oma-applications.path"
fi
put 644 "$src/share/mime/packages/oma.xml" "$mime/packages/oma.xml"
refresh_types
# The handler for a Bundle file (ADR 0097): written with the path of the `odk` installed above,
# and made the default for the type; a release before it has none.
handler=
if [ -f "$src/share/applications/odk-oma.desktop" ] && [ ! -L "$src/share/applications/odk-oma.desktop" ]; then
    mkdir -p "$apps"
    sed "s|@ODK@|\"$bin/odk\"|" "$src/share/applications/odk-oma.desktop" >"$apps/odk-oma.desktop.new.$$"
    chmod 644 "$apps/odk-oma.desktop.new.$$"
    mv -f "$apps/odk-oma.desktop.new.$$" "$apps/odk-oma.desktop"
    say "wrote $apps/odk-oma.desktop"
    refresh_apps
    if command -v awk >/dev/null 2>&1; then
        set_default
    else
        say "note: awk is not installed, so the handler was not made the default for .oma files"
    fi
    handler=yes
fi
# ODK's licence and those of what its programs contain (ADR 0081); a release before them has none.
for doc in LICENSE THIRD-PARTY-LICENSES; do
    if [ -f "$src/share/doc/odk/$doc" ] && [ ! -L "$src/share/doc/odk/$doc" ]; then
        put 644 "$src/share/doc/odk/$doc" "$data/doc/odk/$doc"
    fi
done

# The release installed now, which an older script, or `odk oma self-upgrade`, is held to.
mkdir -p "$(dirname "$receipt")"
printf '# The odk release installed here (ADR 0086), written by install.sh.\nversion = %s\n' \
    "$release_version" >"$receipt.new.$$"
chmod 644 "$receipt.new.$$"
mv -f "$receipt.new.$$" "$receipt"

# --- start --------------------------------------------------------------------------------
if have_user_manager; then
    systemctl --user daemon-reload
    # systemd refuses to start a socket for a service that is already running, and a running
    # odkd has only the sockets it started with, so it is stopped first (ADR 0074). The next
    # connection starts the new program with every socket; odkd.service wants them all, so a
    # connection between these two lines does too.
    systemctl --user stop odkd.service
    # $sockets is one or two unit names, split on purpose.
    systemctl --user enable --now $sockets
    # $registration is none or two unit names, split on purpose.
    [ -z "$registration" ] || systemctl --user enable --now $registration
    started=yes
else
    started=no
    say "note: there is no systemd user session here, so odkd was not started;"
    say "      log in to a session and run: systemctl --user enable --now $sockets"
fi

path_note
say "installed odk $version (release $release_version)"
say "  command:  $bin/odk"
say "  private:  $lib/ (odk-oma, oma-consent, odkd)"
say "  units:    $units/ ($sockets, odkd.service${registration:+, $registration}); started: $started"
if [ -n "$handler" ]; then
    say "  type:     $mime/packages/oma.xml, handler $apps/odk-oma.desktop (a .oma file runs, or asks first)"
else
    say "  type:     $mime/packages/oma.xml (a .oma file opens as a disk image)"
fi
say "  remove:   sh install.sh --uninstall"
