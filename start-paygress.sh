#!/usr/bin/env sh
set -e

# make BASEDIR absolute so ./target/... works even if gateway-cli changes CWD
BASEDIR="$(cd "$(dirname "$0")" && pwd)"

# set env the server requires
export WHITELISTED_MINTS="https://nofees.testnut.cashu.space,https://testnut.cashu.space"
export POD_SPECS_FILE="$BASEDIR/pod-specs.json"
export CASHU_DB_PATH="$BASEDIR/cashu.db"
export POD_NAMESPACE="user-workloads"
export SSH_HOST="localhost"
export BASE_IMAGE="linuxserver/openssh-server:latest"

# replace the shell with your binary (preserves fd/stdin/stdout)
exec "$BASEDIR/target/debug/paygress-mcp-server" "$@"
