use async_watch2::channel;
use tokio_test::task::spawn;
use tokio_test::{assert_pending, assert_ready, assert_ready_err, assert_ready_ok};

// Tests are copied and adapted from [`tokio/tests/sync_watch.rs`](https://github.com/tokio-rs/tokio/blob/4c645866ef4ea5b0ef8c7852281a09b2f96d969b/tokio/tests/sync_watch.rs).

#[test]
fn single_rx_recv() {
    let (tx, mut rx) = channel("one");

    {
        // Not initially notified
        let mut t = spawn(rx.changed());
        assert_pending!(t.poll());
    }
    assert_eq!(*rx.borrow(), "one");

    {
        let mut t = spawn(rx.changed());
        assert_pending!(t.poll());

        tx.send("two").unwrap();

        assert!(t.is_woken());

        assert_ready_ok!(t.poll());
    }
    assert_eq!(*rx.borrow(), "two");

    {
        tx.send("three").unwrap();

        let mut t = spawn(rx.recv());
        let res = assert_ready_ok!(t.poll());
        assert_eq!(res, "three");
    }

    {
        let mut t = spawn(rx.changed());
        assert_pending!(t.poll());

        drop(tx);

        assert!(t.is_woken());
        assert_ready_err!(t.poll());
    }

    assert_eq!(*rx.borrow(), "three");
}

#[test]
fn multi_rx() {
    let (tx, mut rx1) = channel("one");
    let mut rx2 = rx1.clone();

    {
        let mut t1 = spawn(rx1.changed());
        let mut t2 = spawn(rx2.changed());

        assert_pending!(t1.poll());
        assert_pending!(t2.poll());
    }
    assert_eq!(*rx1.borrow(), "one");
    assert_eq!(*rx2.borrow(), "one");

    let mut t2 = spawn(rx2.changed());

    {
        let mut t1 = spawn(rx1.changed());

        assert_pending!(t1.poll());
        assert_pending!(t2.poll());

        tx.send("two").unwrap();

        assert!(t1.is_woken());
        assert!(t2.is_woken());

        assert_ready_ok!(t1.poll());
    }
    assert_eq!(*rx1.borrow(), "two");

    {
        let mut t1 = spawn(rx1.changed());

        assert_pending!(t1.poll());

        tx.send("three").unwrap();

        assert!(t1.is_woken());
        assert!(t2.is_woken());

        assert_ready_ok!(t1.poll());
        assert_ready_ok!(t2.poll());
    }
    assert_eq!(*rx1.borrow(), "three");

    drop(t2);

    assert_eq!(*rx2.borrow(), "three");

    {
        let mut t1 = spawn(rx1.changed());
        let mut t2 = spawn(rx2.changed());

        assert_pending!(t1.poll());
        assert_pending!(t2.poll());

        tx.send("four").unwrap();

        assert_ready_ok!(t1.poll());
        assert_ready_ok!(t2.poll());
    }
    assert_eq!(*rx1.borrow(), "four");
    assert_eq!(*rx2.borrow(), "four");
}

#[test]
fn rx_observes_final_value() {
    // Initial value

    let (tx, mut rx) = channel("one");
    drop(tx);

    {
        let mut t1 = spawn(rx.changed());
        assert_ready_err!(t1.poll());
    }
    assert_eq!(*rx.borrow(), "one");

    // Sending a value

    let (tx, mut rx) = channel("one");

    tx.send("two").unwrap();

    {
        let mut t1 = spawn(rx.changed());
        assert_ready_ok!(t1.poll());
    }
    assert_eq!(*rx.borrow(), "two");

    {
        let mut t1 = spawn(rx.changed());
        assert_pending!(t1.poll());

        tx.send("three").unwrap();
        drop(tx);

        assert!(t1.is_woken());

        assert_ready_ok!(t1.poll());
    }
    assert_eq!(*rx.borrow(), "three");

    {
        let mut t1 = spawn(rx.changed());
        assert_ready_err!(t1.poll());
    }
    assert_eq!(*rx.borrow(), "three");
}

#[test]
fn poll_close() {
    let (tx, rx) = channel("one");

    {
        let mut t = spawn(tx.closed());
        assert_pending!(t.poll());

        drop(rx);

        assert!(t.is_woken());
        assert_ready!(t.poll());
    }

    assert!(tx.send("two").is_err());
}
