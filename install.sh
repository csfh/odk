#!/bin/sh
# Installs odk for the user who runs it (ADR 0063, ADR 0086): the `odk` command, the programs
# behind it, and the `odkd` daemon with its systemd user units, and the `.oma` file type
# (ADR 0065). No root, nothing outside the home directory.
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
# Options:  --system        install under /usr for every user, as root (it never calls sudo)
#           --uninstall     remove what this script installed (application data is kept)
#           --help
#
# Where things go:  ~/.local/bin/odk
#                   ~/.local/lib/odk/{odk-oma,oma-consent,odkd}
#                   ${XDG_CONFIG_HOME:-~/.config}/systemd/user/{odkd.socket,odkd-files.socket,odkd.service,
#                       oma-register.service,oma-applications.path,oma-inbox@.path}
#                   ${XDG_DATA_HOME:-~/.local/share}/mime/packages/oma.xml
#                   ${XDG_DATA_HOME:-~/.local/share}/doc/odk/{LICENSE,THIRD-PARTY-LICENSES}
#                   ${XDG_DATA_HOME:-~/.local/share}/oma/runtime.toml, the release installed
# With --system:    /usr/bin/odk, /usr/lib/odk/, /usr/lib/systemd/user/, /usr/share/mime/,
#                   /usr/share/doc/odk/, the binfmt_misc line /usr/lib/binfmt.d/odk.conf, the
#                   handler /usr/share/applications/oma.desktop, and /var/lib/oma/runtime.toml
#
# Running it again upgrades; an older release than the one installed is refused. `ODK_BASE_URL`
# names another place to download from, and `ODK_DESTDIR` another root to write under, for tests.
set -eu

# --- the release --------------------------------------------------------------------------
# `tools/package-release` writes these when it makes a release, and the release root's signature
# over this script covers them (ADR 0086). In the repository they are empty.
release_version=
tarball_sha256=
tarball_length=

action=install
system=no

say() { printf 'odk install: %s\n' "$*"; }
die() { printf 'odk install: %s\n' "$*" >&2; exit 1; }

while [ $# -gt 0 ]; do
    case $1 in
        --system) system=yes ;;
        --uninstall) action=uninstall ;;
        --help | -h) sed -n '2,28p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) die "unknown option '$1' (try --help)" ;;
    esac
    shift
done

# --- where --------------------------------------------------------------------------------
# Root installs under /usr only when asked, and nothing else does (ADR 0060, section 8).
if [ "$system" = yes ]; then
    [ "$(id -u)" -eq 0 ] \
        || die "--system installs under /usr and needs root; run it as root (it never calls sudo). Nothing was written"
else
    [ "$(id -u)" -ne 0 ] \
        || die "run this as your own user, or with --system to install under /usr. Nothing was written"
fi
root=${ODK_DESTDIR:-}
case $root in '' | /*) ;; *) die "ODK_DESTDIR is not an absolute path" ;; esac
if [ "$system" = yes ]; then
    bin=$root/usr/bin
    lib=$root/usr/lib/odk
    units=$root/usr/lib/systemd/user
    mime=$root/usr/share/mime
    doc=$root/usr/share/doc/odk
    receipt=$root/var/lib/oma/runtime.toml
    binfmt=$root/usr/lib/binfmt.d/odk.conf
    handler=$root/usr/share/applications/oma.desktop
else
    case ${HOME:-} in /*) ;; *) die "HOME is not an absolute path" ;; esac
    bin=$root$HOME/.local/bin
    lib=$root$HOME/.local/lib/odk
    case ${XDG_CONFIG_HOME:-} in /*) config=$XDG_CONFIG_HOME ;; *) config=$HOME/.config ;; esac
    units=$root$config/systemd/user
    case ${XDG_DATA_HOME:-} in /*) data=$XDG_DATA_HOME ;; *) data=$HOME/.local/share ;; esac
    mime=$root$data/mime
    doc=$root$data/doc/odk
    receipt=$root$data/oma/runtime.toml
fi

# The file type is known to the desktop once its database is rebuilt. Without the tool the
# type is in place for when it is there.
refresh_types() {
    if command -v update-mime-database >/dev/null 2>&1; then
        update-mime-database "$mime" || say "note: update-mime-database failed; .oma files may not be recognised yet"
    else
        say "note: update-mime-database (shared-mime-info) is not installed, so .oma files are not recognised yet"
    fi
}

# A user manager is there when it answers. Over ssh with no session bus it does not. A system
# install, or one under another root, has none to start.
have_user_manager() {
    [ "$system" = no ] && [ -z "$root" ] && command -v systemctl >/dev/null 2>&1 \
        && systemctl --user show-environment >/dev/null 2>&1
}

# For every user's session: enabled under the root written to, and nothing is started.
global() { # systemctl verb unit...
    verb=$1
    shift
    if [ -n "$root" ]; then
        systemctl --root="$root" --global "$verb" "$@"
    else
        systemctl --global "$verb" "$@"
    fi
}

# The kernel reads binfmt.d when systemd-binfmt runs; under another root there is no kernel of
# its own to tell.
reload_binfmt() {
    if [ -n "$root" ]; then
        say "note: under $root the binfmt_misc line is in place; systemd-binfmt registers it at boot"
    else
        systemctl restart systemd-binfmt.service || say "note: systemd-binfmt did not restart; the line is registered at the next boot"
    fi
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
    if [ "$system" = yes ]; then
        global disable odkd.socket odkd-files.socket oma-register.service oma-applications.path >/dev/null 2>&1 || true
    fi
    for file in "$bin/odk" "$lib/odk-oma" "$lib/oma-consent" "$lib/odkd" \
        "$units/odkd.socket" "$units/odkd-files.socket" "$units/odkd.service" \
        "$units/oma-register.service" "$units/oma-applications.path" "$units/oma-inbox@.path" \
        "$mime/packages/oma.xml" "$doc/LICENSE" "$doc/THIRD-PARTY-LICENSES" \
        ${binfmt:+"$binfmt"} ${handler:+"$handler"}; do
        if [ -e "$file" ]; then
            rm -f "$file"
            say "removed $file"
        fi
    done
    rmdir "$lib" "$doc" 2>/dev/null || true
    if have_user_manager; then systemctl --user daemon-reload || true; fi
    if [ "$system" = yes ]; then reload_binfmt; fi
    if [ -d "$mime/packages" ]; then refresh_types; fi
    say "done; application data, and the record of the release installed ($receipt), are kept"
    exit 0
fi

# --- checks -------------------------------------------------------------------------------
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
if [ "$system" = yes ]; then
    for file in share/binfmt.d/odk.conf share/applications/oma.desktop; do
        [ -f "$src/$file" ] && [ ! -L "$src/$file" ] || die "the tarball has no $file; nothing was installed"
    done
    # The units name the programs under /usr, not under a home directory.
    for unit in "$src"/share/systemd/user/*; do
        sed -e 's|%h/\.local/lib/odk/|/usr/lib/odk/|g' -e 's|%h/\.local/bin/|/usr/bin/|g' "$unit" >"$unit.system"
        mv -f "$unit.system" "$unit"
    done
fi
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
# ODK's licence and those of what its programs contain (ADR 0081); a release before them has none.
for licence in LICENSE THIRD-PARTY-LICENSES; do
    if [ -f "$src/share/doc/odk/$licence" ] && [ ! -L "$src/share/doc/odk/$licence" ]; then
        put 644 "$src/share/doc/odk/$licence" "$doc/$licence"
    fi
done
if [ "$system" = yes ]; then
    put 644 "$src/share/binfmt.d/odk.conf" "$binfmt"
    put 644 "$src/share/applications/oma.desktop" "$handler"
fi

# The release installed now, which an older script, or `odk oma self-upgrade`, is held to.
mkdir -p "$(dirname "$receipt")"
printf '# The odk release installed here (ADR 0086), written by install.sh.\nversion = %s\n' \
    "$release_version" >"$receipt.new.$$"
chmod 644 "$receipt.new.$$"
mv -f "$receipt.new.$$" "$receipt"

# --- start --------------------------------------------------------------------------------
if [ "$system" = yes ]; then
    # Every user's session starts odkd at its next login; one running now keeps its program.
    global enable $sockets
    [ -z "$registration" ] || global enable $registration
    reload_binfmt
    started="at each user's next login"
elif have_user_manager; then
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

[ "$system" = yes ] || path_note
say "installed odk $version (release $release_version)"
say "  command:  $bin/odk"
say "  private:  $lib/ (odk-oma, oma-consent, odkd)"
say "  units:    $units/ ($sockets, odkd.service${registration:+, $registration}); started: $started"
say "  type:     $mime/packages/oma.xml (a .oma file opens as a disk image)"
if [ "$system" = yes ]; then
    say "  binfmt:   $binfmt; handler: $handler"
    say "  remove:   sh install.sh --system --uninstall"
else
    say "  remove:   sh install.sh --uninstall"
fi
