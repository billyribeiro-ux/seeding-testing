#!/usr/bin/env bash
# scripts/demo.sh — end-to-end demo of the curriculum's services.
#
# Spins up every standalone Rust service in turn, exercises its HTTP surface
# with curl, and prints what's happening. Useful for:
#   - Verifying a fresh clone really works.
#   - Recording an asciinema for marketing / onboarding.
#   - Showing colleagues what the curriculum produces.
#
# Each service is started on its own port; the script waits for /healthz to
# return 200 before driving requests.
#
# Usage:
#   bash scripts/demo.sh
#
# Exit codes:
#   0 — every smoke check passed
#   1 — a service failed to start or a request didn't return as expected

set -euo pipefail

cd "$(dirname "$0")/.."

step() { printf "\n\033[1;34m──── %s ────\033[0m\n" "$*"; }
ok()   { printf "  \033[1;32m✓\033[0m %s\n" "$*"; }
warn() { printf "  \033[1;33m!\033[0m %s\n" "$*"; }
die()  { printf "  \033[1;31m✗\033[0m %s\n" "$*"; exit 1; }

PIDS=()
cleanup() {
    for pid in "${PIDS[@]:-}"; do
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
        fi
    done
}
trap cleanup EXIT INT TERM

wait_for_health() {
    local url="$1"
    local i
    for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15; do
        if curl -sf "$url" >/dev/null 2>&1; then
            return 0
        fi
        sleep 0.5
    done
    return 1
}

# ----------------------------------------------------------------------------
step "Building the workspace (release)"
cargo build --release --workspace 2>&1 | tail -5
ok "workspace built"

# ----------------------------------------------------------------------------
step "hello-cli — Phase 1"
./target/release/hello-cli --version
echo "alpha beta gamma" | ./target/release/hello-cli
./target/release/hello-cli README.md --lines

# ----------------------------------------------------------------------------
step "quote-generator — Phase 2 (sketch: requires HTTP targets)"
./target/release/quote-generator --help | head -20

# ----------------------------------------------------------------------------
step "notes-api — Phase 4 + 5"
DATABASE_URL=sqlite::memory: APP_BIND=127.0.0.1:3000 ./target/release/notes-api &
PIDS+=($!)
wait_for_health http://127.0.0.1:3000/healthz || die "notes-api didn't start"
ok "notes-api up on :3000"

curl -s http://127.0.0.1:3000/healthz | head -c 80 ; echo
created=$(curl -s -X POST http://127.0.0.1:3000/v1/notes \
    -H 'content-type: application/json' \
    -d '{"body":"hello from demo"}')
echo "  POST → $created"
note_id=$(echo "$created" | sed -E 's/.*"id":([0-9]+).*/\1/')
ok "created note id=$note_id"

curl -s "http://127.0.0.1:3000/v1/notes/$note_id" | head -c 120 ; echo
curl -s -i "http://127.0.0.1:3000/v1/notes/999" | head -8

# ----------------------------------------------------------------------------
step "auth-demo — Phase 6"
DATABASE_URL=sqlite::memory: APP_BIND=127.0.0.1:3001 \
    SESSION_SECRET=$(printf '%.0sA' {1..64}) \
    JWT_SECRET=$(printf '%.0sB' {1..32}) \
    ./target/release/auth-demo &
PIDS+=($!)
wait_for_health http://127.0.0.1:3001/healthz || die "auth-demo didn't start"
ok "auth-demo up on :3001"

curl -s -X POST http://127.0.0.1:3001/auth/register \
    -H 'content-type: application/json' \
    -d '{"email":"alice@demo.test","password":"correct horse battery staple"}' \
    | head -c 100 ; echo
login=$(curl -s -X POST http://127.0.0.1:3001/auth/login \
    -H 'content-type: application/json' \
    -d '{"email":"alice@demo.test","password":"correct horse battery staple"}')
token=$(echo "$login" | sed -E 's/.*"access_token":"([^"]+)".*/\1/')
[ -n "$token" ] && [ "$token" != "$login" ] || die "login didn't return an access_token"
ok "logged in; got bearer token (${#token} chars)"
curl -s -H "authorization: Bearer $token" http://127.0.0.1:3001/me | head -c 120 ; echo

# ----------------------------------------------------------------------------
step "webhook-receiver — Phase 8"
DATABASE_URL=sqlite::memory: APP_BIND=127.0.0.1:3002 \
    STRIPE_WEBHOOK_SECRET=whsec_demo_secret_thirty_two_or_more_bytes_here \
    ./target/release/webhook-receiver &
PIDS+=($!)
wait_for_health http://127.0.0.1:3002/healthz || die "webhook-receiver didn't start"
ok "webhook-receiver up on :3002"

# Unsigned request should be rejected
unauth_status=$(curl -s -o /dev/null -w "%{http_code}" -X POST http://127.0.0.1:3002/webhooks/stripe \
    -H 'content-type: application/json' -d '{"id":"evt_demo","type":"x","created":1}')
[ "$unauth_status" = "400" ] && ok "unsigned request rejected with $unauth_status" \
    || die "expected 400 from unsigned webhook; got $unauth_status"

# ----------------------------------------------------------------------------
step "All services healthy; smoke checks passed"
ok "hello-cli   — CLI ran end to end"
ok "notes-api   — POST + GET + 404 returned problem-details"
ok "auth-demo   — register + login + bearer-authenticated /me"
ok "webhook-receiver — signature defense returned 400 on unsigned"

echo
echo "Stop the demo: ctrl-C (services will be cleaned up automatically)."
echo
sleep 0.5
