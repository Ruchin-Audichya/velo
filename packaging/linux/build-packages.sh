#!/usr/bin/env bash
# Package the three Linux binaries as a .deb (Ubuntu, Debian, Mint), a .rpm
# (Fedora, RHEL-likes, openSUSE) and a portable .tar.gz.
#
#   packaging/linux/build-packages.sh <version> <dir with velo, velod, velo-cli> <out dir>
#
# Needs dpkg-deb, dpkg-shlibdeps and rpmbuild (`apt install dpkg-dev rpm`).
#
# All three binaries go to /usr/bin side by side: the window starts the
# daemon from beside its own executable, so they must stay together.
set -euo pipefail

version=${1:?version}
bin_dir=${2:?directory holding velo, velod and velo-cli}
out_dir=${3:?output directory}

case "$version" in
    *[!A-Za-z0-9._-]* | "" | -* | .*)
        echo "refusing a version that is not a plain name: $version" >&2
        exit 1
        ;;
esac
version=${version#v}

here=$(cd -- "$(dirname -- "$0")" && pwd)
repo=$(cd -- "$here/../.." && pwd)
app_id=io.github.RuchinAudichya.Velo
arch_deb=amd64
arch_rpm=x86_64
case "$(uname -m)" in
    aarch64 | arm64) arch_deb=arm64; arch_rpm=aarch64 ;;
esac

mkdir -p "$out_dir"
out_dir=$(cd -- "$out_dir" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

for binary in velo velod velo-cli; do
    [ -x "$bin_dir/$binary" ] || { echo "missing executable: $bin_dir/$binary" >&2; exit 1; }
done

# One file tree, shared by both packages.
root="$work/root"
install -Dm755 "$bin_dir/velo" "$root/usr/bin/velo"
install -Dm755 "$bin_dir/velod" "$root/usr/bin/velod"
install -Dm755 "$bin_dir/velo-cli" "$root/usr/bin/velo-cli"
install -Dm644 "$here/$app_id.png" "$root/usr/share/icons/hicolor/256x256/apps/$app_id.png"
install -Dm644 "$here/$app_id.desktop" "$root/usr/share/applications/$app_id.desktop"
install -Dm644 "$here/$app_id.metainfo.xml" "$root/usr/share/metainfo/$app_id.metainfo.xml"
install -Dm644 "$repo/LICENSE" "$root/usr/share/doc/velo/copyright"
install -Dm644 "$repo/THIRD-PARTY-NOTICES.txt" "$root/usr/share/doc/velo/THIRD-PARTY-NOTICES.txt"

summary="Fast, native WhatsApp client for the desktop"
description="Velo is an unofficial WhatsApp client written in Rust and drawn on the GPU, with no embedded browser. It links to your phone as a linked device, like WhatsApp Web. Not affiliated with WhatsApp or Meta."
# Loaded at run time rather than linked, so no tool can find them: the GPU
# (Vulkan), and the Wayland and X11 keyboard libraries.
deb_runtime="libvulkan1, libwayland-client0, libxkbcommon0, libxkbcommon-x11-0"
rpm_runtime="vulkan-loader libwayland-client libxkbcommon libxkbcommon-x11"

# --- .deb -------------------------------------------------------------------
deb="$work/deb"
cp -a "$root" "$deb"
mkdir -p "$deb/DEBIAN" "$work/shlibs/debian"
printf 'Source: velo\n\nPackage: velo\nArchitecture: any\n' > "$work/shlibs/debian/control"
linked=$(cd "$work/shlibs" && dpkg-shlibdeps -O "$deb/usr/bin/velo" "$deb/usr/bin/velod" "$deb/usr/bin/velo-cli" 2>/dev/null \
    | sed -n 's/^shlibs:Depends=//p')
size=$(du -sk "$deb/usr" | cut -f1)
cat > "$deb/DEBIAN/control" <<EOF
Package: velo
Version: $version
Architecture: $arch_deb
Maintainer: Ruchin Audichya <77974751+Ruchin-Audichya@users.noreply.github.com>
Installed-Size: $size
Depends: ${linked:+$linked, }$deb_runtime
Recommends: mesa-vulkan-drivers
Section: net
Priority: optional
Homepage: https://github.com/Ruchin-Audichya/velo
Description: $summary
 $description
EOF
deb_file="$out_dir/velo_${version}_${arch_deb}.deb"
dpkg-deb --root-owner-group --build "$deb" "$deb_file" >/dev/null
echo "built $deb_file"

# --- .rpm -------------------------------------------------------------------
rpm_top="$work/rpm"
mkdir -p "$rpm_top"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
cat > "$rpm_top/SPECS/velo.spec" <<EOF
%global debug_package %{nil}
%global __strip /bin/true
Name:           velo
Version:        $(printf '%s' "$version" | tr '-' '~')
Release:        1
Summary:        $summary
License:        MIT
URL:            https://github.com/Ruchin-Audichya/velo
Requires:       $rpm_runtime
Recommends:     mesa-vulkan-drivers

%description
$description

%install
cp -a "$root/." %{buildroot}/

%files
/usr/bin/velo
/usr/bin/velod
/usr/bin/velo-cli
/usr/share/applications/$app_id.desktop
/usr/share/icons/hicolor/256x256/apps/$app_id.png
/usr/share/metainfo/$app_id.metainfo.xml
%license /usr/share/doc/velo/copyright
%doc /usr/share/doc/velo/THIRD-PARTY-NOTICES.txt
EOF
rpmbuild --quiet --define "_topdir $rpm_top" --target "$arch_rpm" -bb "$rpm_top/SPECS/velo.spec"
rpm_file="$out_dir/velo-${version}-1.${arch_rpm}.rpm"
mv "$rpm_top"/RPMS/*/velo-*.rpm "$rpm_file"
echo "built $rpm_file"

# --- portable .tar.gz -------------------------------------------------------
tar_name="velo-${version}-linux-$(uname -m)"
mkdir -p "$work/$tar_name"
cp "$bin_dir/velo" "$bin_dir/velod" "$bin_dir/velo-cli" "$here/install-desktop.sh" "$here/$app_id.png" "$work/$tar_name/"
chmod +x "$work/$tar_name/"{velo,velod,velo-cli,install-desktop.sh}
tar -C "$work" -czf "$out_dir/$tar_name.tar.gz" "$tar_name"
echo "built $out_dir/$tar_name.tar.gz"
