#!/bin/sh
# Installs odk for the user who runs it (ADR 0063): the `odk` command, the programs behind it, and
# the `odkd` daemon with its systemd user units, and the `.oma` file type (ADR 0065). No root,
# nothing outside the home directory.
#
#   curl --proto '=https' --tlsv1.2 -fsSL https://github.com/csfh/odk/releases/latest/download/install.sh | sh
#
# The same, saved and read first:
#
#   curl --proto '=https' --tlsv1.2 -fsSL -o install.sh https://github.com/csfh/odk/releases/latest/download/install.sh
#   sh install.sh
#
# Options:  --version TAG   install that release instead of the latest
#           --uninstall     remove what this script installed (application data is kept)
#           --help
#
# Where things go:  ~/.local/bin/odk
#                   ~/.local/lib/odk/{odk-oma,oma-consent,odkd}
#                   ${XDG_CONFIG_HOME:-~/.config}/systemd/user/{odkd.socket,odkd-files.socket,odkd.service,
#                       oma-register.service,oma-applications.path,oma-inbox@.path}
#                   ${XDG_DATA_HOME:-~/.local/share}/mime/packages/oma.xml
#
# Running it again upgrades. `ODK_BASE_URL` names another place to download from, for tests.
set -eu

repo=https://github.com/csfh/odk/releases
tag=
action=install

say() { printf 'odk install: %s\n' "$*"; }
die() { printf 'odk install: %s\n' "$*" >&2; exit 1; }

while [ $# -gt 0 ]; do
    case $1 in
        --uninstall) action=uninstall ;;
        --version)
            [ $# -ge 2 ] || die "--version needs a release tag"
            tag=$2
            shift
            ;;
        --help | -h) sed -n '2,22p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) die "unknown option '$1' (try --help)" ;;
    esac
    shift
done

# --- where --------------------------------------------------------------------------------
[ "$(id -u)" -ne 0 ] || die "run this as your own user, not as root: it installs under your home directory"
case ${HOME:-} in /*) ;; *) die "HOME is not an absolute path" ;; esac
prefix=$HOME/.local
bin=$prefix/bin
lib=$prefix/lib/odk
case ${XDG_CONFIG_HOME:-} in /*) config=$XDG_CONFIG_HOME ;; *) config=$HOME/.config ;; esac
units=$config/systemd/user
case ${XDG_DATA_HOME:-} in /*) data=$XDG_DATA_HOME ;; *) data=$HOME/.local/share ;; esac
mime=$data/mime

# The file type is known to the desktop once its database is rebuilt. Without the tool the
# type is in place for when it is there.
refresh_types() {
    if command -v update-mime-database >/dev/null 2>&1; then
        update-mime-database "$mime" || say "note: update-mime-database failed; .oma files may not be recognised yet"
    else
        say "note: update-mime-database (shared-mime-info) is not installed, so .oma files are not recognised yet"
    fi
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
        "$mime/packages/oma.xml"; do
        if [ -e "$file" ]; then
            rm -f "$file"
            say "removed $file"
        fi
    done
    rmdir "$lib" 2>/dev/null || true
    if have_user_manager; then systemctl --user daemon-reload || true; fi
    if [ -d "$mime/packages" ]; then refresh_types; fi
    say "done; application data under ~/.local/share/oma is kept"
    exit 0
fi

# --- checks -------------------------------------------------------------------------------
[ "$(uname -s)" = Linux ] || die "odk runs on Linux only"
[ "$(uname -m)" = x86_64 ] || die "odk is built for x86_64 only"
for tool in curl tar gzip sha256sum mktemp install; do
    command -v "$tool" >/dev/null 2>&1 || die "needs '$tool' and did not find it"
done

# --- download -----------------------------------------------------------------------------
name=odk-x86_64-linux
if [ -n "${ODK_BASE_URL:-}" ]; then
    base=$ODK_BASE_URL
elif [ -n "$tag" ]; then
    base=$repo/download/$tag
else
    base=$repo/latest/download
fi
# Only HTTPS, or a local file for tests. A redirect is followed, but only to HTTPS: a release
# download is a redirect to GitHub's CDN.
case $base in
    https://*) fetch="curl --proto =https --proto-redir =https --tlsv1.2 -fsSL" ;;
    file://*) fetch="curl --proto =file -fsS" ;;
    *) die "the download address must be https:// (or file:// for a test): $base" ;;
esac

work=$(mktemp -d "${TMPDIR:-/tmp}/odk-install.XXXXXX")
chmod 700 "$work"
trap 'rm -rf "$work"' EXIT HUP INT TERM

say "downloading $base/$name.tar.gz"
$fetch -o "$work/$name.tar.gz" "$base/$name.tar.gz" || die "cannot download $base/$name.tar.gz"
$fetch -o "$work/$name.tar.gz.sha256" "$base/$name.tar.gz.sha256" \
    || die "cannot download $base/$name.tar.gz.sha256"

# The checksum says the download is the file the release published. It does not say who
# published it: that is the trust in GitHub over HTTPS (ADR 0063).
# The line names the file it is the sum of; it must name this tarball, or a sum of some other
# file would pass without the tarball being looked at.
read -r want named <"$work/$name.tar.gz.sha256" || die "the checksum file is empty; nothing was installed"
[ "${named#\*}" = "$name.tar.gz" ] \
    || die "the checksum is for '$named', not $name.tar.gz; nothing was installed"
have=$(sha256sum "$work/$name.tar.gz" | cut -d' ' -f1)
[ -n "$want" ] && [ "$have" = "$want" ] \
    || die "the download does not match its checksum; nothing was installed"
say "checksum ok"

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
say "installed odk $version"
say "  command:  $bin/odk"
say "  private:  $lib/ (odk-oma, oma-consent, odkd)"
say "  units:    $units/ ($sockets, odkd.service${registration:+, $registration}); started: $started"
say "  type:     $mime/packages/oma.xml (a .oma file opens as a disk image)"
say "  remove:   sh install.sh --uninstall"
