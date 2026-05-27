//! Integration tests — exercise the crate through its public API only.
//!
//! These tests sit outside the crate so they confirm the API shape is
//! usable from the outside: aggregates, the event store, and the
//! projection runner all interoperate without crate-private helpers.

use std::time::Duration;

use event_sourcing::{
    AccountStatus, Aggregate, BalanceProjection, BankAccount, BankCommand, BankEvent, EsError,
    EventStore, run_projection,
};

/// End-to-end: open → deposit → withdraw → close. The aggregate
/// driver code looks the way real callers would write it.
#[tokio::test]
async fn end_to_end_account_lifecycle() {
    let store = EventStore::in_memory().await.unwrap();
    let stream_id = "acct-life";

    // Drive a sequence of commands. Each iteration: load → handle →
    // append at expected_version → done. This is the canonical
    // event-sourcing command loop.
    for cmd in [
        BankCommand::OpenAccount { owner_id: 99 },
        BankCommand::Deposit { cents: 500 },
        BankCommand::Deposit { cents: 1_500 },
        BankCommand::Withdraw { cents: 250 },
        BankCommand::Close,
    ] {
        let acc: BankAccount = store.load_aggregate(stream_id).await.unwrap();
        let expected_version = store.current_version(stream_id).await.unwrap();
        let events = acc.handle(cmd).unwrap();
        store
            .append(stream_id, expected_version, &events)
            .await
            .unwrap();
    }

    let final_state: BankAccount = store.load_aggregate(stream_id).await.unwrap();
    assert_eq!(final_state.status, AccountStatus::Closed);
    assert_eq!(final_state.balance_cents, 1_750);
    assert_eq!(final_state.owner_id, Some(99));
}

/// A realistic retry loop: when we hit `ConcurrencyConflict`, reload
/// the stream and retry. This is how real applications cope with the
/// optimistic-concurrency model; the test proves the API supports it
/// cleanly.
#[tokio::test]
async fn conflict_then_retry_succeeds() {
    let store = EventStore::in_memory().await.unwrap();
    let stream_id = "acct-retry";

    // Set up: account exists at version 1.
    store
        .append(stream_id, 0, &[BankEvent::Opened { owner_id: 1 }])
        .await
        .unwrap();

    // First writer wins.
    store
        .append(stream_id, 1, &[BankEvent::Deposited { cents: 100 }])
        .await
        .unwrap();

    // Second writer's stale append fails.
    let err = store
        .append(stream_id, 1, &[BankEvent::Deposited { cents: 999 }])
        .await
        .unwrap_err();
    assert!(matches!(err, EsError::ConcurrencyConflict { .. }));

    // Reload + retry.
    let acc: BankAccount = store.load_aggregate(stream_id).await.unwrap();
    let version = store.current_version(stream_id).await.unwrap();
    let evs = acc.handle(BankCommand::Deposit { cents: 999 }).unwrap();
    store.append(stream_id, version, &evs).await.unwrap();

    let final_state: BankAccount = store.load_aggregate(stream_id).await.unwrap();
    assert_eq!(final_state.balance_cents, 1_099);
}

/// Two independent accounts in two streams have independent versions
/// and projections that aggregate them correctly.
#[tokio::test]
async fn multi_stream_projection_keeps_balances_separate() {
    let store = EventStore::in_memory().await.unwrap();
    let projection = BalanceProjection::new(&store).await.unwrap();

    store
        .append(
            "alice",
            0,
            &[
                BankEvent::Opened { owner_id: 1 },
                BankEvent::Deposited { cents: 1_000 },
            ],
        )
        .await
        .unwrap();
    store
        .append(
            "bob",
            0,
            &[
                BankEvent::Opened { owner_id: 2 },
                BankEvent::Deposited { cents: 2_500 },
                BankEvent::Withdrew { cents: 500 },
            ],
        )
        .await
        .unwrap();

    let handle = tokio::spawn({
        let store = store.clone();
        let projection = projection.clone();
        async move {
            let _ = tokio::time::timeout(
                Duration::from_millis(500),
                run_projection(
                    store,
                    projection,
                    Duration::from_millis(10),
                    100,
                    std::future::pending::<()>(),
                ),
            )
            .await;
        }
    });

    // Wait for both balances to converge.
    let deadline = std::time::Instant::now() + Duration::from_millis(400);
    loop {
        let a = projection.balance("alice").await.unwrap();
        let b = projection.balance("bob").await.unwrap();
        if a == Some(1_000) && b == Some(2_000) {
            break;
        }
        assert!(
            std::time::Instant::now() <= deadline,
            "did not converge; alice={a:?} bob={b:?}",
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    handle.abort();
    let _ = handle.await;
}
