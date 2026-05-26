//! Inlined policy predicates adapted from `projects/05-rbac-policy-lab/`.
//!
//! Each `pub fn can_*` answers a permission question for a (subject, resource)
//! pair. Admin override comes first, then specific allows, then deny last.
//! The wire response for every denial is a generic 403 — the server logs the
//! distinct `Forbidden` variant so we can debug without leaking authz shape
//! to clients.

use thiserror::Error;

use crate::auth::User;
use crate::notes::Note;

/// Every distinct denial reason. The variant is logged + audit-trailed; the
/// HTTP response carries only a generic problem-details body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum Forbidden {
    #[error("not the owner")]
    NotOwner,
    #[error("wrong tenant")]
    WrongTenant,
    #[error("tier too low")]
    InsufficientTier,
    #[error("not published")]
    NotPublished,
}

pub type PolicyResult = Result<(), Forbidden>;

/// Early-return helper. Mirrors the macro in rbac-policy-lab.
#[macro_export]
macro_rules! require {
    ($result:expr) => {
        match $result {
            Ok(()) => (),
            Err(reason) => return Err(reason),
        }
    };
}

// ---------------------------------------------------------------------------
// Role / tier helpers
// ---------------------------------------------------------------------------

/// True for moderators and admins.
fn is_privileged(user: &User) -> bool {
    matches!(user.role.as_str(), "moderator" | "admin")
}

fn is_admin(user: &User) -> bool {
    user.role == "admin"
}

/// Tier ordering: free < pro < elite. Returns `0..=2`.
fn tier_rank(tier: &str) -> u8 {
    match tier {
        "pro" => 1,
        "elite" => 2,
        _ => 0,
    }
}

fn require_same_tenant_or_admin(user: &User, resource_org_id: i64) -> PolicyResult {
    if is_admin(user) || user.org_id == resource_org_id {
        return Ok(());
    }
    Err(Forbidden::WrongTenant)
}

// ---------------------------------------------------------------------------
// Document policies
// ---------------------------------------------------------------------------

/// Can the user read this note? Admins can read anything. Same-tenant users
/// can read either their own (drafts included) or anyone's published note,
/// provided their tier meets the note's `min_tier`.
pub fn can_read_doc(user: &User, note: &Note) -> PolicyResult {
    if is_admin(user) {
        return Ok(());
    }
    require_same_tenant_or_admin(user, note.org_id)?;
    if note.published_at.is_none() && note.owner_id != user.id {
        return Err(Forbidden::NotPublished);
    }
    if tier_rank(&user.tier) < tier_rank(&note.min_tier) {
        return Err(Forbidden::InsufficientTier);
    }
    Ok(())
}

/// Can the user write to this note? Admins can. The owner can. A same-tenant
/// moderator can. Everyone else is denied.
pub fn can_write_doc(user: &User, note: &Note) -> PolicyResult {
    if is_admin(user) {
        return Ok(());
    }
    if note.owner_id == user.id {
        return Ok(());
    }
    if is_privileged(user) && user.org_id == note.org_id {
        return Ok(());
    }
    Err(Forbidden::NotOwner)
}

/// Can the user delete this note? Admins can; the owner can; nobody else.
pub fn can_delete_doc(user: &User, note: &Note) -> PolicyResult {
    if is_admin(user) {
        return Ok(());
    }
    if note.owner_id == user.id {
        return Ok(());
    }
    Err(Forbidden::NotOwner)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(id: i64, role: &str, tier: &str, org: i64) -> User {
        User {
            id,
            email: format!("u{id}@example.com"),
            password_hash: String::new(),
            role: role.to_string(),
            tier: tier.to_string(),
            org_id: org,
            email_verified: 0,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn note(owner_id: i64, org: i64, min_tier: &str, published: bool) -> Note {
        Note {
            id: 1,
            owner_id,
            org_id: org,
            body: "test".to_string(),
            published_at: if published {
                Some("2026-05-26T12:00:00.000Z".to_string())
            } else {
                None
            },
            min_tier: min_tier.to_string(),
            created_at: String::new(),
        }
    }

    #[test]
    fn owner_can_read_own_draft() {
        let u = user(1, "member", "free", 1);
        let n = note(1, 1, "free", false);
        assert_eq!(can_read_doc(&u, &n), Ok(()));
    }

    #[test]
    fn other_member_cannot_read_unpublished() {
        let u = user(2, "member", "free", 1);
        let n = note(1, 1, "free", false);
        assert_eq!(can_read_doc(&u, &n), Err(Forbidden::NotPublished));
    }

    #[test]
    fn admin_overrides_owner_check() {
        let u = user(2, "admin", "free", 1);
        let n = note(1, 1, "free", false);
        assert_eq!(can_read_doc(&u, &n), Ok(()));
    }

    #[test]
    fn cross_tenant_blocked() {
        let u = user(2, "member", "free", 7);
        let n = note(1, 1, "free", true);
        assert_eq!(can_read_doc(&u, &n), Err(Forbidden::WrongTenant));
    }

    #[test]
    fn tier_gate_enforced() {
        let u = user(2, "member", "free", 1);
        let n = note(1, 1, "pro", true);
        assert_eq!(can_read_doc(&u, &n), Err(Forbidden::InsufficientTier));
    }

    #[test]
    fn moderator_can_write_in_tenant() {
        let u = user(2, "moderator", "free", 1);
        let n = note(1, 1, "free", true);
        assert_eq!(can_write_doc(&u, &n), Ok(()));
    }

    #[test]
    fn member_cannot_delete_another_users_note() {
        let u = user(2, "member", "free", 1);
        let n = note(1, 1, "free", true);
        assert_eq!(can_delete_doc(&u, &n), Err(Forbidden::NotOwner));
    }
}
