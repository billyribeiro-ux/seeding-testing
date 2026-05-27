# 10-memberclub-cli

The Rust CLI client the curriculum plan promised: "We ship a `memberclub`
CLI in Rust that uses [the JWT bearer flow]." Authenticates against the
MemberClub HTTP API, stashes the returned JWTs in
`$XDG_CONFIG_HOME/memberclub/credentials.toml`, then drives every other
subcommand off that stored token.

## Subcommands

```bash
memberclub login --api-base-url http://127.0.0.1:3001 \
                 --email alice@example.test \
                 --password "correct horse battery staple"

memberclub whoami       # prints email, verified?, admin?, totp_enabled?
memberclub notes list --limit 5
memberclub notes list --cursor <opaque>
memberclub logout
```

`--config-dir <path>` is a global override for tests + sandboxes —
without it the CLI reads/writes the platform-standard config home.

## Security notes

  * Credentials file is `chmod 600` after every save (Unix).
  * `--password` reads from `MEMBERCLUB_PASSWORD` if set; the env var
    keeps it out of shell history.
  * `logout` calls `/auth/logout` *before* clearing the local file, so
    a stolen token can't survive a logout race.

## Tests

`tests/cli.rs` drives the real binary against a `wiremock::MockServer`:
4 hermetic e2e tests cover login (round-trip + file shape), whoami
(bearer-token is read from the file), logout (cookie revoked +
file cleared + idempotent re-logout), and notes list (page rendering +
`--next-cursor=` emission).

`src/config.rs` carries 4 unit tests for the file round-trip and the
Unix permission bits.
