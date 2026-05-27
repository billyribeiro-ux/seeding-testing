# Disaster Recovery

> The on-call playbook for the worst day.

This is the runbook you reach for when something is gone — the primary
database is unrecoverable, a region is dark, a critical secret has leaked
and you have to rotate everything, or somebody dropped a table at 2am.

It is written for a half-asleep on-call engineer. Read top-to-bottom.
Do not skip steps.

For routine operational issues, see the other runbooks:
- [`5xx-spike.md`](./5xx-spike.md) — the app is throwing 5xx
- [`db-connection-storms.md`](./db-connection-storms.md) — connection pool exhaustion
- [`deploy-failure.md`](./deploy-failure.md) — bad deploy
- [`rollback.md`](./rollback.md) — rolling back a release
- [`stripe-webhook-lag.md`](./stripe-webhook-lag.md) — webhook queue backlog

If you're not sure which runbook applies, **page the on-call lead** and
let them decide.

## Recovery objectives

These targets define what "good enough" looks like for MemberClub.
Numbers are aligned with the perf budgets in
[`../perf/budgets.md`](../perf/budgets.md).

| Component | RPO (data we can afford to lose) | RTO (time we can be down) |
|---|---|---|
| Primary Postgres database | 5 minutes (continuous WAL archival) | 60 minutes |
| Redis cache | 24 hours (it's a cache; loss is OK) | 30 minutes (cold start is fast) |
| JWT signing keys | 0 (rotated, not lost — see secret rotation below) | 15 minutes |
| Stripe webhook secret | 0 (rotated) | 15 minutes |
| Application binaries / Docker images | 0 (in registry; multi-region) | 10 minutes |
| Object storage (user uploads) | 0 (cross-region replication) | n/a (read-only during failover) |

RPO = Recovery Point Objective = "we will lose at most X minutes of data."
RTO = Recovery Time Objective = "we will be back up within Y minutes."

These are *targets*, not promises. The status page should commit to looser
numbers (we don't tell customers "60 minutes" out loud).

## What we need to recover

### Postgres data (the only thing that really matters)

- **Source of truth.** Everything else can be rebuilt from this.
- Backups: `pg_dump` nightly + continuous WAL archive (PITR-capable).
- Backup location: `s3://memberclub-backups-primary/postgres/` (primary
  region) and `s3://memberclub-backups-secondary/postgres/` (replica
  region via S3 Cross-Region Replication).
- Retention: 30 days hot in S3 Standard, 365 days cold in Glacier.

### Redis state

- Mostly transient: session cache, rate-limit counters, recent-results
  cache.
- Persistent-ish: idempotency keys (24-hour TTL anyway), Stripe webhook
  dedupe table (24-hour TTL).
- **OK to lose entirely.** A cold Redis after disaster gives degraded
  performance for ~15 minutes while the cache warms.

### JWT signing keys

- Private RS256 keys live in AWS Secrets Manager (`prod/memberclub/jwt/private`).
- Public keys are published at `/.well-known/jwks.json`.
- See [`projects/04-auth-demo/src/jwt_rs256.rs`](../../projects/04-auth-demo/src/jwt_rs256.rs)
  for the implementation.
- Rotation: every 90 days, with overlapping validity (old key remains
  in JWKS for verification for 30 days after rotation).

### Stripe webhook secret

- Lives in AWS Secrets Manager (`prod/memberclub/stripe/webhook_secret`).
- Used by [`projects/07-webhook-receiver`](../../projects/07-webhook-receiver/)
  to verify webhook signatures.
- Rotated in Stripe Dashboard; new value pushed to Secrets Manager
  before old value is invalidated.

### Database password

- Lives in AWS Secrets Manager (`prod/memberclub/postgres/password`).
- App reads at startup; rotation requires either app restart or
  IAM-based authentication (preferred for future).

### Application binary

- Docker image in ECR.
- Multi-region replicated automatically.
- Rebuilding from source: `git clone` + CI run. Source of truth is
  GitHub.

### TLS certificates

- ACM-managed; auto-renewed.
- If ACM is down (cosmically unlikely but possible), serve from
  CloudFront's edge certs which have a 90-day cache.

## Backup strategy

### Nightly `pg_dump`

A cron job (Kubernetes CronJob) runs at 02:00 UTC:

```bash
pg_dump \
  --host=$PG_HOST \
  --username=$BACKUP_USER \
  --format=custom \
  --verbose \
  --file=/tmp/dump-$(date +%Y%m%d).pgdump \
  memberclub

aws s3 cp /tmp/dump-$(date +%Y%m%d).pgdump \
  s3://memberclub-backups-primary/postgres/nightly/ \
  --sse aws:kms --sse-kms-key-id $BACKUP_KMS_KEY_ID
```

The dump is verified by a separate restore-test job that runs weekly
(see "Testing restore" below). Without restore testing, "we have
backups" is a hope, not a control.

### Continuous WAL archive

RDS Postgres has `archive_mode = on` and `archive_command` pointing to
S3 via the `wal2json` extension. Every WAL segment (typically 16MB) is
shipped to:

```
s3://memberclub-backups-primary/postgres/wal/<timeline>/<segment>
```

Cross-region replication mirrors to:

```
s3://memberclub-backups-secondary/postgres/wal/<timeline>/<segment>
```

with a typical replication lag of seconds.

### Retention windows

| Backup tier | Storage class | Retention |
|---|---|---|
| Last 30 days of nightlies + WAL | S3 Standard | 30 days |
| 31–365 days of monthlies | S3 Glacier Instant Retrieval | 11 months |
| 1–7 years (financial / legal hold) | S3 Glacier Deep Archive | 6 years |

### Testing restore

Restore is verified every week by an automated job that:

1. Spins up a fresh RDS instance.
2. Restores the latest nightly dump.
3. Replays WAL up to the last hour.
4. Runs a smoke-test query suite (row counts per table within ±10% of
   prod, key constraints valid, a few business invariants).
5. Tears down the instance.
6. Posts the result to `#dr-tests` Slack channel.

If the restore-test alert is red for more than 48 hours, escalate
**before** you need to actually restore in anger.

## The actual restore procedure (primary DB unrecoverable)

This is the procedure for the canonical disaster: production Postgres
is gone — corrupted, deleted, in a region that won't come back.

The on-call engineer should be paired with at least one other engineer
on a video call. **Do not do this alone.**

1. **Declare the incident.** Page the on-call lead. Open the incident
   channel. Post initial status to the status page: "Investigating an
   issue affecting all customers." Start the postmortem timeline
   immediately — capture every command and decision with timestamps.

2. **Stop the application traffic.** Scale the API deployment to zero
   replicas:

   ```
   kubectl scale deploy/memberclub-api -n prod --replicas=0
   ```

   This prevents partial-state reads/writes against whatever DB you
   end up with. Maintenance page (CloudFront-served static) takes
   over.

3. **Confirm the primary really is gone.** Don't panic-restore on top
   of a healthy primary. Check the RDS console; check
   `pg_stat_activity` from a bastion; check the actual error you're
   responding to. If the primary is alive but read-only or
   degraded, this is the wrong runbook — go to
   [`5xx-spike.md`](./5xx-spike.md) first.

4. **Identify the recovery target time.** What's the latest point we
   can recover *to*? Two cases:
   - The DB died at time T due to hardware/region failure: target = T
     (or as close as WAL archive lets us).
   - The DB died at time T due to data corruption / bad migration:
     target = the last known-good time *before* T.

   Write the target time down. Communicate to the room. Do not change
   it without consensus.

5. **Find the most recent base backup before the target time.** List
   the S3 bucket:

   ```
   aws s3 ls s3://memberclub-backups-primary/postgres/nightly/
   ```

   Pick the most recent dump strictly before the target time.

6. **Provision the new RDS instance.** Use the Terraform module in
   `infra/dr/restore-instance` (not in this repo):

   ```
   cd infra/dr/restore-instance
   terraform apply -var target_time=$TARGET_TIME -var base_backup=$BASE_BACKUP
   ```

   This creates an empty RDS instance with the same instance class as
   prod, sized identically, in the same VPC. ~10 minutes to provision.

7. **Restore the base backup.** From a maintenance bastion in the new
   instance's VPC:

   ```
   pg_restore \
     --host=$NEW_PG_HOST \
     --username=$RESTORE_USER \
     --dbname=memberclub \
     --no-owner \
     --verbose \
     /tmp/$BASE_BACKUP
   ```

   This will take time proportional to the database size — 15 minutes
   for a 50GB DB on the rehearsal we ran, longer for larger.

8. **Restore WAL segments up to the target time.** Configure recovery:

   ```
   echo "restore_command = 'aws s3 cp s3://memberclub-backups-primary/postgres/wal/13/%f %p'" \
     >> postgresql.auto.conf
   echo "recovery_target_time = '$TARGET_TIME'" \
     >> postgresql.auto.conf
   echo "recovery_target_action = 'pause'" \
     >> postgresql.auto.conf
   ```

   (RDS-managed restore exposes this through the AWS Console — use
   the console for RDS. The above is for self-managed Postgres or
   for understanding what RDS is doing under the hood.)

9. **Start Postgres in recovery mode.** It will read WAL segments from
   S3 and replay them up to the target time, then pause.

10. **Verify the recovered state.** From psql:

    ```sql
    SELECT now();
    SELECT max(created_at) FROM users;
    SELECT count(*) FROM subscriptions WHERE status = 'active';
    SELECT max(id) FROM audit_log;
    ```

    Compare against your memory of "what these numbers should look
    like." If they're wildly wrong, do not promote — go back and
    investigate.

11. **Promote.** Once verified, exit recovery:

    ```sql
    SELECT pg_wal_replay_resume();
    -- then in shell:
    pg_ctl promote
    ```

    The instance becomes writable. Verify with a manual `INSERT` and
    `ROLLBACK`.

12. **Rotate the database password.** The DB came back; the credentials
    that touched the old DB should not touch the new one. See
    "Secret rotation" below.

13. **Update DNS / service discovery.** Point the application's
    `DATABASE_URL` at the new instance. We use AWS Secrets Manager
    + IRSA, so:

    ```
    aws secretsmanager update-secret \
      --secret-id prod/memberclub/postgres/host \
      --secret-string $NEW_PG_HOST
    ```

    Then rolling-restart the API deployment:

    ```
    kubectl rollout restart deploy/memberclub-api -n prod
    ```

14. **Bring traffic back up gradually.** Scale to 1 replica first;
    verify metrics; then scale to full capacity:

    ```
    kubectl scale deploy/memberclub-api -n prod --replicas=1
    # wait, watch dashboards
    kubectl scale deploy/memberclub-api -n prod --replicas=20
    ```

15. **Update the status page and the incident channel.** "Service
    restored. Investigation continues. Postmortem to follow."

## The corruption scenario

Different from "primary is gone": "primary is alive but its data is
wrong." Examples:

- A bad migration set `users.email = NULL` for half the table.
- An attacker ran `UPDATE` statements that altered balances.
- A storage-layer bug silently corrupted a small set of pages.

This is harder than the "primary is gone" case because:

- The corrupted primary is the source of subsequent commits. If you
  roll forward via WAL from a known-good base, you replay the
  corruption.
- You can't simply restore a backup that contains the corruption
  either.

The procedure:

1. **Stop traffic immediately.** Every second more of writes makes the
   recovery harder. Scale API to zero.

2. **Determine the corruption time window.** When was the last
   known-good state? Use audit log, monitoring data, customer
   reports.

3. **Decide the recovery strategy:**
   - **Surgical**: if the corruption is bounded (e.g. "rows in
     `users` where `id BETWEEN 1000 AND 2000`"), keep the primary
     and selectively restore the affected rows from a backup.
     Verify exhaustively.
   - **Full rewind**: if corruption is unbounded or untrusted, do a
     full PITR to a time before the corruption. You will lose
     post-corruption writes. Communicate this clearly.

4. **For surgical**: provision a parallel restore instance (steps 6–11
   of the main procedure) restored to a known-good time. Dump the
   affected tables/rows from the parallel instance. Truncate or
   delete the bad rows in primary. Insert from the dump. Audit
   diff.

5. **For full rewind**: do the main procedure with target time =
   pre-corruption.

6. **Communicate honestly.** "Between 14:00 and 14:30 UTC, changes
   you made may not have been preserved" is a hard message but it is
   the right message. Hiding data loss multiplies the eventual
   damage.

### What NOT to do during corruption

- Do NOT `DELETE` or `UPDATE` against the corrupted primary trying to
  "fix" things by hand. You will introduce new corruption.
- Do NOT take a fresh `pg_dump` of the corrupted primary thinking
  it's a backup. It's a backup of corruption.
- Do NOT roll the application forward in the hope that newer code
  will "skip" the bad data. New code will trip on the bad data in
  different ways.

## The region-failure scenario

The entire primary region is dark. AWS `us-east-1` is having a Bad
Day. We are not auto-failed-over (we deliberately don't have automated
cross-region failover at our scale — see the trade-off below).

1. **Confirm the region is actually down.** Don't failover for a flap.
   AWS Status Page, internal monitoring, and at least 5 minutes of
   consistent failure.

2. **Decide to fail over.** This is a one-way trip until the primary
   region is back AND we manually fail back. Page leadership before
   triggering.

3. **Activate the secondary-region stack.** Terraform-managed; this
   exists as a "warm spare" — RDS instance running, application
   deployment scaled to zero. Promote and scale:

   ```
   # Promote the cross-region read replica to primary
   aws rds promote-read-replica \
     --db-instance-identifier memberclub-prod-replica-secondary \
     --region $SECONDARY_REGION

   # Scale the secondary-region app
   kubectl --context=secondary scale deploy/memberclub-api -n prod --replicas=20
   ```

4. **Switch DNS.** Update Route 53 weighted records to send 100% of
   traffic to the secondary region. TTL is 60 seconds; expect a few
   minutes of mixed traffic.

5. **Verify and watch dashboards.** Cross-region failover changes
   latency profile; some queries will be slower because data has
   different physical layout.

### The trade-off vs auto-failover

We chose **manual** cross-region failover. The reasoning:

- **Auto-failover is hard to make right.** False positives (the
  primary is slow, not dead) cause unnecessary failovers that
  themselves cause incidents. The classic "split brain" problem.
- **At our scale, 30 minutes of downtime is tolerable.** Our RTO is
  60 minutes; manual failover fits in that budget if the runbook is
  rehearsed. Auto-failover's value is when RTO is single-digit
  minutes.
- **Manual gives humans a chance to assess.** "Should we failover?"
  is a real question — sometimes the better answer is "wait, the
  region's coming back."

This trade-off is reconsidered annually. If the company grows enough
that 30 minutes of downtime costs more than the engineering effort
of safe auto-failover, we'll build it. Today, the math doesn't
favor it.

## Secret rotation as part of restore

Any time we touch the database in disaster recovery, we should assume
the credentials have been compromised. Even if we don't have evidence
of compromise, rotating is cheap insurance.

### Rotate database password

```
NEW_PW=$(openssl rand -base64 48)

aws secretsmanager update-secret \
  --secret-id prod/memberclub/postgres/password \
  --secret-string "$NEW_PW"

psql -h $NEW_PG_HOST -U postgres -c "ALTER USER memberclub WITH PASSWORD '$NEW_PW';"

kubectl rollout restart deploy/memberclub-api -n prod
```

### Rotate JWT signing key

Generate new RS256 key, add to JWKS as a second valid key, then
deprecate the old:

```
openssl genrsa -out new_private.pem 2048
openssl rsa -in new_private.pem -pubout -out new_public.pem

# Add new private to Secrets Manager
aws secretsmanager update-secret \
  --secret-id prod/memberclub/jwt/private \
  --secret-string file://new_private.pem

# Both old and new public keys remain in JWKS for 30 days
# Update the JWKS publisher to include both, mark new as "current"
```

The application will sign new tokens with the new key. Old tokens
still verify against the old public key for 30 days. After that, the
old key is removed from JWKS. See
[`projects/04-auth-demo/src/jwt_rs256.rs`](../../projects/04-auth-demo/src/jwt_rs256.rs)
for the rotating-key logic.

### Rotate Stripe webhook secret

1. In the Stripe Dashboard → Developers → Webhooks → your endpoint,
   click "Roll secret." This generates a new secret and starts a
   24-hour window during which both old and new secrets are valid.
2. Update Secrets Manager:

   ```
   aws secretsmanager update-secret \
     --secret-id prod/memberclub/stripe/webhook_secret \
     --secret-string "whsec_NEW_VALUE"
   ```

3. Restart the webhook receiver:

   ```
   kubectl rollout restart deploy/memberclub-webhook-receiver -n prod
   ```

4. After verification, the old secret expires naturally at the end of
   the 24-hour overlap.

## Post-incident checklist

The incident is over. Now the work to make sure it doesn't happen
again — or that if it does, we handle it better.

- [ ] Postmortem opened in [`../03-postmortems/`](../03-postmortems/),
      using the [`0000-TEMPLATE.md`](../03-postmortems/0000-TEMPLATE.md).
      The [`2026-05-26-notes-api-5xx-spike-EXAMPLE.md`](../03-postmortems/2026-05-26-notes-api-5xx-spike-EXAMPLE.md)
      shows the level of detail expected.
- [ ] Status-page incident closed with a final summary.
- [ ] Customer-facing communication (email, blog post) drafted and
      reviewed by legal if data was affected.
- [ ] Action items captured as tickets, owned, with deadlines.
- [ ] Compliance assessment: was this a notifiable breach under
      GDPR / HIPAA / state law? See
      [`../compliance/01-gdpr.md`](../compliance/01-gdpr.md) Article
      33 timeline. If yes, regulator notification within 72 hours.
- [ ] If secrets were rotated: confirm all consumers picked up the
      new values. Look for stragglers in logs.
- [ ] If we failed over regions: schedule the failback plan.
- [ ] DR runbook update: did anything in this document turn out to
      be wrong? Fix it now while you remember.
- [ ] Restore-test job: did it catch this? If not, why not? Update
      the restore-test to verify the failure mode that bit us.
- [ ] Schedule a tabletop exercise within 30 days to walk through
      this runbook with the broader team.

## Practicing this runbook

Once a quarter, the on-call rotation runs a **tabletop exercise**: a
designated facilitator picks one of the scenarios (primary
unrecoverable / corruption / region failure) and walks the on-call
through the runbook step-by-step on a non-prod instance. Real
commands, real timing, no shortcuts.

The first time you run this runbook, it should not be in production
at 3am. Practice it first.
