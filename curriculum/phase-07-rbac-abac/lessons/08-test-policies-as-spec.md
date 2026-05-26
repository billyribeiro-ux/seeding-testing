# Lesson 7.8 — Policies as the Test Suite is the Spec

> **Concept first:** every policy gets at least one *allow* test and one *deny* test. The test file becomes the security spec — anyone reading it learns "what can a member do?" without reading code.
> **Time:** 15 minutes.

## The pattern

```rust
#[test]
fn admin_can_delete_any_doc() {
    let admin = user().admin().build();
    let doc   = doc().owner_id(99).build();
    assert!(policy::can_delete_doc(&admin, &doc, &Ctx::test()).is_ok());
}

#[test]
fn member_cannot_delete_others_docs() {
    let member = user().member().id(42).build();
    let doc    = doc().owner_id(99).build();
    assert_eq!(
        policy::can_delete_doc(&member, &doc, &Ctx::test()).unwrap_err(),
        Forbidden::NotOwner
    );
}

#[test]
fn owner_can_delete_own_doc() {
    let member = user().member().id(42).build();
    let doc    = doc().owner_id(42).build();
    assert!(policy::can_delete_doc(&member, &doc, &Ctx::test()).is_ok());
}
```

Three tests per policy: admin path, owner path, denied path. Every variant of `Forbidden` gets a test that surfaces it. The file reads like a permission matrix.

## The factory helpers

The tests above lean on tiny builders so each test is *one line of setup*:

```rust
fn user() -> UserBuilder { UserBuilder::default() }
fn doc()  -> DocBuilder  { DocBuilder::default() }
```

```rust
struct UserBuilder { id: i64, role: Role, tier: Tier, org_id: i64, ... }
impl UserBuilder {
    fn admin(mut self) -> Self { self.role = Role::Admin; self }
    fn member(mut self) -> Self { self.role = Role::Member; self }
    fn id(mut self, id: i64) -> Self { self.id = id; self }
    fn build(self) -> User { ... }
}
```

These are *test-only* builders. They live in a `test_support` module that's only compiled under `#[cfg(test)]`.

## Coverage rule of thumb

For each `pub fn can_X(...) -> Result<(), Forbidden>`:

- One test per `return Ok(())` path (each independent allow reason).
- One test per `return Err(...)` path (each distinct deny reason).
- One *boundary* test where useful (`tier == doc.min_tier` — allow; `tier == doc.min_tier - 1` — deny).

Coverage of `policy.rs` should be 100%. It's pure logic; there's no excuse.

## Property tests for invariants

Some policies have invariants worth property-testing:

```rust
proptest! {
    /// Promoting any user to admin gives them every permission.
    #[test]
    fn admin_has_all_permissions(perm in arbitrary_permission()) {
        let admin = user().admin().build();
        assert!(rbac::has_permission(&admin, perm));
    }

    /// Admin override never depends on tenancy.
    #[test]
    fn admin_can_read_doc_in_any_org(my_org in 1i64..100, doc_org in 1i64..100) {
        let admin = user().admin().org_id(my_org).build();
        let doc   = doc().org_id(doc_org).build();
        prop_assert!(policy::can_read_doc(&admin, &doc, &Ctx::test()).is_ok());
    }
}
```

These catch the case you didn't think of.

## Naming the tests

`<actor_role>_<verb>_<object>_<modifier>`:

- `admin_can_delete_any_doc`
- `member_cannot_delete_others_docs`
- `owner_can_delete_own_doc`
- `moderator_cannot_delete_doc_in_other_org`

The test name reads like a sentence in the spec. Don't abbreviate.

## Integration tests over the HTTP layer

The unit tests above are pure-function. We also write integration tests that:

- Authenticate as a particular role.
- Hit the actual endpoint.
- Assert the right status (`403` with the right title) or success.

Integration tests catch a different bug class: "we forgot to put `require!` in the handler." They're slower; we keep fewer of them, focused on the *most damaging* missed checks.

## Why this matters

- **The test file is the spec.** A new engineer can read `policy_tests.rs` and learn everything the system permits.
- **Per-variant coverage** ensures every `Forbidden` variant is *tested into existence*.
- **Properties catch the case you didn't enumerate.** Admin-bypass, tenant-isolation — these are invariants, not cases.

## Green-bar checkpoint

- You can write three tests for one policy (admin allow, owner allow, deny).
- You can write a proptest for "admin is omnipotent."
- You can name a test in the `<actor>_<verb>_<object>_<modifier>` style.

Next: `lessons/09-build-rbac-policy-lab.md` (the capstone walkthrough).
