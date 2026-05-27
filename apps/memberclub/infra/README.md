# MemberClub — infra/

Container images and orchestration for the MemberClub capstone.

| What | Where |
|---|---|
| Dev Postgres + Redis + MailHog services | `compose.yaml` at the repo root |
| API image | `apps/memberclub/infra/Dockerfile.api` |
| Web image (SvelteKit adapter-node) | `apps/memberclub/infra/Dockerfile.web` |
| Production compose for self-host | `apps/memberclub/infra/compose.prod.yaml` |
| GitHub Actions CI | `.github/workflows/ci.yml` |
| GitHub Actions release | `.github/workflows/release.yml` |

## Why two compose files

`compose.yaml` (root) is for **development** — it binds ports to
`127.0.0.1`, mounts the working tree, and runs hot-reload. Never deploy it
to production.

`compose.prod.yaml` (here) is the **single-box reference deployment** —
the same images CI publishes (api + web + Postgres + Redis), reverse-
proxied through Caddy with automatic HTTPS, and a separate `db` volume
that's the source of truth for backup scripts.

## Deploying

The curriculum documents three deploy targets in
`docs/01-architecture-decisions/`:

  * Fly.io for the API (`flyctl deploy`)
  * Vercel for the SvelteKit web app (`vercel --prod`)
  * Neon for managed Postgres (mount `DATABASE_URL` from secrets)

The Dockerfiles here are portable across all of them — they build a
multi-stage image that ends in a `~50 MiB distroless` runtime layer.

## Building locally

```bash
docker build -f apps/memberclub/infra/Dockerfile.api -t memberclub-api:dev .
docker build -f apps/memberclub/infra/Dockerfile.web -t memberclub-web:dev .

# Run the whole prod stack on this host:
docker compose -f apps/memberclub/infra/compose.prod.yaml up -d
```
