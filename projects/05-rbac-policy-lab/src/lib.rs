//! rbac-policy-lab — a pure-policy library.
//!
//! Demonstrates the enterprise pattern for combining RBAC and ABAC into a
//! single set of testable predicate functions. The library exposes:
//!
//!   * Domain types: User, Document, Role, Tier
//!   * Forbidden enum naming every distinct denial reason
//!   * `pub fn can_X(...)` predicate functions
//!   * `require!` macro that turns a policy result into early-return
//!   * Test-only builders so each test is a one-line setup
//!
//! The test file (`tests/policy.rs`) reads like a permission matrix — anyone
//! browsing the test names learns what the system permits and denies.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Domain
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Member,
    Moderator,
    Admin,
    Owner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Tier {
    Free,
    Pro,
    Elite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrgId(pub i64);

#[derive(Debug, Clone)]
pub struct User {
    pub id: UserId,
    pub email: String,
    pub email_verified: bool,
    pub role: Role,
    pub tier: Tier,
    pub org_id: OrgId,
    /// When the user last completed a TOTP check. `None` means never.
    pub totp_last_verified_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct Document {
    pub id: i64,
    pub owner_id: UserId,
    pub org_id: OrgId,
    pub min_tier: Tier,
    pub published_at: Option<DateTime<Utc>>,
}

/// Per-request context: the wall-clock time and request metadata.
#[derive(Debug, Clone)]
pub struct Ctx {
    pub now: DateTime<Utc>,
}

impl Ctx {
    #[must_use]
    pub fn at(now: DateTime<Utc>) -> Self {
        Self { now }
    }
}

// ---------------------------------------------------------------------------
// Forbidden — every distinct denial reason. Server logs the reason; the wire
// response is a generic 403.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum Forbidden {
    #[error("not authenticated")]
    NotAuthenticated,
    #[error("email not verified")]
    EmailNotVerified,
    #[error("not the owner")]
    NotOwner,
    #[error("not an admin")]
    NotAdmin,
    #[error("not a moderator or admin")]
    NotModerator,
    #[error("wrong tenant")]
    WrongTenant,
    #[error("tier too low")]
    InsufficientTier,
    #[error("not published")]
    NotPublished,
    #[error("step-up authentication required")]
    StepUpRequired,
    #[error("cannot remove the last admin")]
    LastAdmin,
}

pub type PolicyResult = Result<(), Forbidden>;

impl User {
    #[must_use]
    pub fn is_admin(&self) -> bool {
        matches!(self.role, Role::Admin | Role::Owner)
    }
    #[must_use]
    pub fn is_moderator_or_above(&self) -> bool {
        matches!(self.role, Role::Moderator | Role::Admin | Role::Owner)
    }
}

// ---------------------------------------------------------------------------
// Reusable building blocks
// ---------------------------------------------------------------------------

pub fn require_role(s: &User, min: Role) -> PolicyResult {
    let ok = match min {
        Role::Member => true,
        Role::Moderator => s.is_moderator_or_above(),
        Role::Admin => s.is_admin(),
        Role::Owner => matches!(s.role, Role::Owner),
    };
    if ok {
        Ok(())
    } else {
        match min {
            Role::Admin | Role::Owner => Err(Forbidden::NotAdmin),
            Role::Moderator => Err(Forbidden::NotModerator),
            Role::Member => Err(Forbidden::NotAuthenticated),
        }
    }
}

pub fn require_email_verified(s: &User) -> PolicyResult {
    if s.email_verified {
        Ok(())
    } else {
        Err(Forbidden::EmailNotVerified)
    }
}

pub fn require_step_up(s: &User, ctx: &Ctx, max_age: Duration) -> PolicyResult {
    let last = s.totp_last_verified_at.ok_or(Forbidden::StepUpRequired)?;
    if ctx.now.signed_duration_since(last) > max_age {
        return Err(Forbidden::StepUpRequired);
    }
    Ok(())
}

pub fn require_same_tenant_or_admin(s: &User, resource_org_id: OrgId) -> PolicyResult {
    if s.is_admin() {
        return Ok(());
    }
    if s.org_id == resource_org_id {
        return Ok(());
    }
    Err(Forbidden::WrongTenant)
}

// ---------------------------------------------------------------------------
// Document policies — one `pub fn can_*` per action.
// Admin override first; specific allows next; deny last.
// ---------------------------------------------------------------------------

pub fn can_read_doc(s: &User, doc: &Document, _ctx: &Ctx) -> PolicyResult {
    if s.is_admin() {
        return Ok(());
    }
    require_same_tenant_or_admin(s, doc.org_id)?;
    if doc.published_at.is_none() && doc.owner_id != s.id {
        return Err(Forbidden::NotPublished);
    }
    if s.tier < doc.min_tier {
        return Err(Forbidden::InsufficientTier);
    }
    Ok(())
}

pub fn can_write_doc(s: &User, doc: &Document, _ctx: &Ctx) -> PolicyResult {
    if s.is_admin() {
        return Ok(());
    }
    if doc.owner_id == s.id {
        require_email_verified(s)?;
        return Ok(());
    }
    if s.is_moderator_or_above() && s.org_id == doc.org_id {
        return Ok(());
    }
    Err(Forbidden::NotOwner)
}

pub fn can_delete_doc(s: &User, doc: &Document, ctx: &Ctx) -> PolicyResult {
    if s.is_admin() {
        // Even admin deletes require a fresh step-up to prevent session-hijack drift.
        return require_step_up(s, ctx, Duration::minutes(15));
    }
    if doc.owner_id == s.id {
        require_email_verified(s)?;
        return Ok(());
    }
    Err(Forbidden::NotOwner)
}

pub fn can_publish_doc(s: &User, doc: &Document, _ctx: &Ctx) -> PolicyResult {
    require_role(s, Role::Moderator)?;
    require_same_tenant_or_admin(s, doc.org_id)?;
    require_email_verified(s)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Admin / role-management policies
// ---------------------------------------------------------------------------

pub fn can_grant_role(
    actor: &User,
    target_role: Role,
    target_user_org_id: OrgId,
    ctx: &Ctx,
) -> PolicyResult {
    require_role(actor, Role::Admin)?;
    if !matches!(actor.role, Role::Owner) && matches!(target_role, Role::Owner) {
        // Only an existing Owner can mint a new Owner.
        return Err(Forbidden::NotAdmin);
    }
    require_same_tenant_or_admin(actor, target_user_org_id)?;
    require_step_up(actor, ctx, Duration::minutes(15))?;
    Ok(())
}

/// Guardrail: refuse to remove the last admin. `remaining_admin_count` is the
/// number of admins that would *still* exist after the removal.
pub fn can_revoke_admin(
    actor: &User,
    target_user_org_id: OrgId,
    remaining_admin_count: usize,
    ctx: &Ctx,
) -> PolicyResult {
    require_role(actor, Role::Admin)?;
    require_same_tenant_or_admin(actor, target_user_org_id)?;
    require_step_up(actor, ctx, Duration::minutes(15))?;
    if remaining_admin_count == 0 {
        return Err(Forbidden::LastAdmin);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// `require!` macro — early-return on a denial.
// ---------------------------------------------------------------------------

#[macro_export]
macro_rules! require {
    ($result:expr) => {
        match $result {
            Ok(()) => (),
            Err(reason) => return Err(reason),
        }
    };
    ($cond:expr, $reason:expr) => {
        if !$cond {
            return Err($reason);
        }
    };
}

// ---------------------------------------------------------------------------
// Test-only builders. Compiled only under #[cfg(test)] from elsewhere by
// importing this module directly in integration tests.
// ---------------------------------------------------------------------------

pub mod test_support {
    #![allow(clippy::wildcard_imports)]
    use super::*;

    #[must_use]
    pub fn user_builder() -> UserBuilder {
        UserBuilder::default()
    }

    #[must_use]
    pub fn doc_builder() -> DocBuilder {
        DocBuilder::default()
    }

    pub struct UserBuilder {
        pub id: UserId,
        pub email: String,
        pub email_verified: bool,
        pub role: Role,
        pub tier: Tier,
        pub org_id: OrgId,
        pub totp_last_verified_at: Option<DateTime<Utc>>,
    }

    impl Default for UserBuilder {
        fn default() -> Self {
            Self {
                id: UserId(1),
                email: "alice@example.com".to_string(),
                email_verified: true,
                role: Role::Member,
                tier: Tier::Free,
                org_id: OrgId(1),
                totp_last_verified_at: Some(Utc::now()),
            }
        }
    }

    impl UserBuilder {
        #[must_use]
        pub fn id(mut self, id: i64) -> Self {
            self.id = UserId(id);
            self
        }
        #[must_use]
        pub fn role(mut self, r: Role) -> Self {
            self.role = r;
            self
        }
        #[must_use]
        pub fn member(self) -> Self {
            self.role(Role::Member)
        }
        #[must_use]
        pub fn moderator(self) -> Self {
            self.role(Role::Moderator)
        }
        #[must_use]
        pub fn admin(self) -> Self {
            self.role(Role::Admin)
        }
        #[must_use]
        pub fn owner(self) -> Self {
            self.role(Role::Owner)
        }
        #[must_use]
        pub fn tier(mut self, t: Tier) -> Self {
            self.tier = t;
            self
        }
        #[must_use]
        pub fn org(mut self, id: i64) -> Self {
            self.org_id = OrgId(id);
            self
        }
        #[must_use]
        pub fn unverified(mut self) -> Self {
            self.email_verified = false;
            self
        }
        #[must_use]
        pub fn step_up_stale(mut self) -> Self {
            self.totp_last_verified_at = Some(Utc::now() - Duration::hours(1));
            self
        }
        #[must_use]
        pub fn no_totp_ever(mut self) -> Self {
            self.totp_last_verified_at = None;
            self
        }
        #[must_use]
        pub fn build(self) -> User {
            User {
                id: self.id,
                email: self.email,
                email_verified: self.email_verified,
                role: self.role,
                tier: self.tier,
                org_id: self.org_id,
                totp_last_verified_at: self.totp_last_verified_at,
            }
        }
    }

    pub struct DocBuilder {
        pub id: i64,
        pub owner_id: UserId,
        pub org_id: OrgId,
        pub min_tier: Tier,
        pub published_at: Option<DateTime<Utc>>,
    }

    impl Default for DocBuilder {
        fn default() -> Self {
            Self {
                id: 1,
                owner_id: UserId(99),
                org_id: OrgId(1),
                min_tier: Tier::Free,
                published_at: Some(Utc::now()),
            }
        }
    }

    impl DocBuilder {
        #[must_use]
        pub fn id(mut self, id: i64) -> Self {
            self.id = id;
            self
        }
        #[must_use]
        pub fn owner(mut self, id: i64) -> Self {
            self.owner_id = UserId(id);
            self
        }
        #[must_use]
        pub fn org(mut self, id: i64) -> Self {
            self.org_id = OrgId(id);
            self
        }
        #[must_use]
        pub fn min_tier(mut self, t: Tier) -> Self {
            self.min_tier = t;
            self
        }
        #[must_use]
        pub fn draft(mut self) -> Self {
            self.published_at = None;
            self
        }
        #[must_use]
        pub fn published(mut self) -> Self {
            self.published_at = Some(Utc::now());
            self
        }
        #[must_use]
        pub fn build(self) -> Document {
            Document {
                id: self.id,
                owner_id: self.owner_id,
                org_id: self.org_id,
                min_tier: self.min_tier,
                published_at: self.published_at,
            }
        }
    }

    #[must_use]
    pub fn ctx_now() -> Ctx {
        Ctx::at(Utc::now())
    }
}
