#!/usr/bin/env bash
# adapted from batna-app main: scripts/ci-apt-install.sh
# Install apt packages on a GitHub-hosted Ubuntu runner (with no arguments it
# only points apt at a working mirror and refreshes the lists, for steps such
# as `playwright install --with-deps` that call apt themselves).
# On 2026-11-11 the runners could not resolve azure.archive.ubuntu.com (the
# image's mirror, listed in /etc/apt/apt-mirrors.txt) nor archive.ubuntu.com,
# and apt hung jobs for an hour. This picks the first Ubuntu mirror whose name
# resolves, points every apt source at it, and time-boxes each try; a failed
# try moves on to the next mirror.
set -euo pipefail

mirrors=(
  http://azure.archive.ubuntu.com/ubuntu
  http://archive.ubuntu.com/ubuntu
  http://mirrors.edge.kernel.org/ubuntu
  http://mirrors.mit.edu/ubuntu
)
opts=(-o Acquire::http::Timeout=20 -o Acquire::Retries=1)

point_apt_at() {
  sudo find /etc/apt -type f \( -name '*.txt' -o -name '*.list' -o -name '*.sources' \) \
    -exec sed -E -i "s#https?://(azure\.archive|archive|security)\.ubuntu\.com/ubuntu|https?://mirrors\.(edge\.kernel\.org|mit\.edu)/ubuntu#$1#g" {} +
}

for mirror in "${mirrors[@]}"; do
  host=${mirror#http://}
  host=${host%%/*}
  if ! getent hosts "$host" >/dev/null; then
    echo "::warning::$host does not resolve; trying the next mirror"
    continue
  fi
  point_apt_at "$mirror"
  if sudo timeout 240 apt-get "${opts[@]}" update &&
    sudo timeout 600 apt-get "${opts[@]}" install -y "$@"; then
    exit 0
  fi
  echo "::warning::apt via $host failed; trying the next mirror"
done
echo "::error::no Ubuntu mirror worked"
exit 1
