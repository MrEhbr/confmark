#!/bin/sh
# confmark installer — downloads a prebuilt binary from GitHub Releases.
#
#   curl -fsSL https://raw.githubusercontent.com/MrEhbr/confmark/main/install.sh | sh
#
# Options (pass after `-s --` when piping, e.g. `... | sh -s -- --version v1.2.3`):
#   --version <tag>   release tag to install (default: latest)
#   --bin-dir <dir>   install destination (default: $CONFMARK_BIN_DIR, else
#                     /usr/local/bin if writable, else ~/.local/bin)
set -eu

REPO="MrEhbr/confmark"
BIN="confmark"
VERSION="latest"
BIN_DIR="${CONFMARK_BIN_DIR:-}"

while [ $# -gt 0 ]; do
	case "$1" in
	--version)
		VERSION="${2:?--version needs a value}"
		shift 2
		;;
	--bin-dir)
		BIN_DIR="${2:?--bin-dir needs a value}"
		shift 2
		;;
	-h | --help)
		sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'
		exit 0
		;;
	*)
		echo "unknown option: $1" >&2
		exit 1
		;;
	esac
done

die() {
	echo "error: $*" >&2
	exit 1
}

case "$(uname -s)" in
Linux) os="Linux" ;;
Darwin) os="Darwin" ;;
*) die "unsupported OS: $(uname -s) (prebuilt binaries cover Linux and macOS)" ;;
esac

case "$(uname -m)" in
x86_64 | amd64) arch="x86_64" ;;
aarch64 | arm64) arch="arm64" ;;
*) die "unsupported architecture: $(uname -m)" ;;
esac

asset="${BIN}_${os}_${arch}.tar.gz"
if [ "$VERSION" = "latest" ]; then
	base="https://github.com/${REPO}/releases/latest/download"
else
	base="https://github.com/${REPO}/releases/download/${VERSION}"
fi

if command -v curl >/dev/null 2>&1; then
	fetch() { curl -fsSL "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then
	fetch() { wget -qO "$2" "$1"; }
else
	die "need curl or wget"
fi

if [ -z "$BIN_DIR" ]; then
	if [ -w /usr/local/bin ]; then
		BIN_DIR="/usr/local/bin"
	else
		BIN_DIR="$HOME/.local/bin"
	fi
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

echo "Downloading ${asset} (${VERSION})"
fetch "${base}/${asset}" "${tmp}/${asset}" || die "download failed: ${base}/${asset}"

# Best-effort checksum verification (GoReleaser split checksums: <asset>.sha256).
if fetch "${base}/${asset}.sha256" "${tmp}/${asset}.sha256" 2>/dev/null; then
	expected="$(awk '{print $1}' "${tmp}/${asset}.sha256")"
	if command -v sha256sum >/dev/null 2>&1; then
		actual="$(sha256sum "${tmp}/${asset}" | awk '{print $1}')"
	elif command -v shasum >/dev/null 2>&1; then
		actual="$(shasum -a 256 "${tmp}/${asset}" | awk '{print $1}')"
	else
		actual=""
	fi
	if [ -n "$actual" ] && [ "$expected" != "$actual" ]; then
		die "checksum mismatch (expected $expected, got $actual)"
	fi
	[ -n "$actual" ] && echo "Checksum verified"
else
	echo "warning: checksum not found, skipping verification" >&2
fi

tar -xzf "${tmp}/${asset}" -C "$tmp"
[ -f "${tmp}/${BIN}" ] || die "binary '${BIN}' not found in archive"

mkdir -p "$BIN_DIR"
install -m 0755 "${tmp}/${BIN}" "${BIN_DIR}/${BIN}" 2>/dev/null ||
	{ cp "${tmp}/${BIN}" "${BIN_DIR}/${BIN}" && chmod 0755 "${BIN_DIR}/${BIN}"; }

echo "Installed ${BIN} to ${BIN_DIR}/${BIN}"
case ":${PATH}:" in
*":${BIN_DIR}:"*) ;;
*) echo "note: ${BIN_DIR} is not on your PATH — add it to use '${BIN}' directly" ;;
esac
