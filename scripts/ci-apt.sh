#!/bin/sh
# Installs Ubuntu packages on a GitHub runner, as `apt-get install` would, but past a mirror
# that cannot be reached: the runner's own Azure mirror at times fails to resolve for a while.
#
#   scripts/ci-apt.sh PACKAGE...
#
# 1. The packages a run before fetched are kept in APT_DEBS, which the workflow caches.
#    Installed from there alone, a run needs no mirror at all.
# 2. Otherwise each mirror is tried in turn, the runner's own first and then Ubuntu's own, each
#    with apt's retries, and what is fetched lands in APT_DEBS for the next run.
set -eu
debs=${APT_DEBS:-/var/tmp/apt-debs}
sudo mkdir -p "$debs/partial"
# left readable for the cache to save, without the download area apt keeps to itself
trap 'sudo rm -rf "$debs/partial" "$debs/lock"; sudo chown -R "$(id -u):$(id -g)" "$debs"' EXIT

installed() {
	[ "$(dpkg-query -W -f='${Status}\n' "$@" 2>/dev/null | grep -c '^install ok installed$')" -eq "$#" ]
}

if installed "$@"; then
	echo "ci-apt: installed already"
	exit 0
fi

if ls "$debs"/*.deb >/dev/null 2>&1; then
	sudo apt-get install -y -q --no-install-recommends "$debs"/*.deb >/dev/null 2>&1 || true
	if installed "$@"; then
		echo "ci-apt: installed from the cache, no mirror needed"
		exit 0
	fi
	echo "ci-apt: the cache did not cover it, going to the mirrors"
fi

case "$(dpkg --print-architecture)" in
amd64) mirrors="http://archive.ubuntu.com/ubuntu http://us.archive.ubuntu.com/ubuntu http://mirrors.edge.kernel.org/ubuntu" ;;
*) mirrors="http://ports.ubuntu.com/ubuntu-ports" ;;
esac

apt="sudo apt-get -o Acquire::Retries=3 -o Acquire::http::Timeout=20 -o APT::Sandbox::User=root -o Dir::Cache::Archives=$debs"
fetch() {
	$apt update -q && $apt install -y -q --no-install-recommends "$@"
}

# A mirror is tried only once it answers in a few seconds: one whose name does not resolve
# would otherwise hold each of apt's retries for its whole timeout.
suite=$(. /etc/os-release && echo "$VERSION_CODENAME")
answers() {
	curl -fsS -m 10 -o /dev/null "$1/dists/$suite/Release"
}
own=$(grep -m1 -E '^https?://' /etc/apt/apt-mirrors.txt 2>/dev/null | cut -f1 | cut -d' ' -f1 || true)
own=${own%/}
if [ -z "$own" ] || answers "$own"; then
	if fetch "$@"; then
		exit 0
	fi
else
	echo "ci-apt: the runner's mirror $own does not answer"
fi
for m in $mirrors; do
	if ! answers "$m"; then
		echo "ci-apt: $m does not answer either"
		continue
	fi
	echo "ci-apt: going to $m"
	# the runner's Ubuntu sources name its mirror list; each is pointed at this mirror instead,
	# and other sources, as Microsoft's, are left as they are
	for f in /etc/apt/sources.list /etc/apt/sources.list.d/*.sources /etc/apt/sources.list.d/*.list; do
		[ -f "$f" ] || continue
		sudo sed -i -E \
			-e "s#^(URIs:[[:space:]]*)(mirror\+file:|https?://[^[:space:]]*ubuntu\.com)[^[:space:]]*#\1$m#" \
			-e "s#^(deb(-src)?([[:space:]]+\[[^]]*\])?[[:space:]]+)(mirror\+file:|https?://[^[:space:]]*ubuntu\.com)[^[:space:]]*#\1$m#" \
			"$f"
	done
	if fetch "$@"; then
		exit 0
	fi
done
echo "::error::ci-apt: no mirror could be reached for $*"
exit 1
