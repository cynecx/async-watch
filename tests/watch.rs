use async_watch2::channel;
use std::cell::Cell;
use std::rc::Rc;
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

// Copied from tokio/tests/async_send_sync.rs

fn is_send_sync<T: Send + Sync>() {}
fn is_clone<T: Clone>() {}

#[allow(dead_code)]
fn require_send<T: Send>(_t: &T) {}
#[allow(dead_code)]
fn require_sync<T: Sync>(_t: &T) {}

#[allow(dead_code)]
struct Invalid;

trait AmbiguousIfSend<A> {
    fn some_item(&self) {}
}
impl<T: ?Sized> AmbiguousIfSend<()> for T {}
impl<T: ?Sized + Send> AmbiguousIfSend<Invalid> for T {}

trait AmbiguousIfSync<A> {
    fn some_item(&self) {}
}
impl<T: ?Sized> AmbiguousIfSync<()> for T {}
impl<T: ?Sized + Sync> AmbiguousIfSync<Invalid> for T {}

macro_rules! into_todo {
    ($typ:ty) => {{
        let x: $typ = todo!();
        x
    }};
}

macro_rules! async_assert_fn {
    ($($f:ident $(< $($generic:ty),* > )? )::+($($arg:ty),*): Send & Sync) => {
        #[allow(unreachable_code)]
        #[allow(unused_variables)]
        const _: fn() = || {
            let f = $($f $(::<$($generic),*>)? )::+( $( into_todo!($arg) ),* );
            require_send(&f);
            require_sync(&f);
        };
    };
    ($($f:ident $(< $($generic:ty),* > )? )::+($($arg:ty),*): Send & !Sync) => {
        #[allow(unreachable_code)]
        #[allow(unused_variables)]
        const _: fn() = || {
            let f = $($f $(::<$($generic),*>)? )::+( $( into_todo!($arg) ),* );
            require_send(&f);
            AmbiguousIfSync::some_item(&f);
        };
    };
    ($($f:ident $(< $($generic:ty),* > )? )::+($($arg:ty),*): !Send & Sync) => {
        #[allow(unreachable_code)]
        #[allow(unused_variables)]
        const _: fn() = || {
            let f = $($f $(::<$($generic),*>)? )::+( $( into_todo!($arg) ),* );
            AmbiguousIfSend::some_item(&f);
            require_sync(&f);
        };
    };
    ($($f:ident $(< $($generic:ty),* > )? )::+($($arg:ty),*): !Send & !Sync) => {
        #[allow(unreachable_code)]
        #[allow(unused_variables)]
        const _: fn() = || {
            let f = $($f $(::<$($generic),*>)? )::+( $( into_todo!($arg) ),* );
            AmbiguousIfSend::some_item(&f);
            AmbiguousIfSync::some_item(&f);
        };
    };
}

macro_rules! assert_not_impl {
    ($x:ty, $($t:path),+ $(,)*) => {
        const _: fn() -> () = || {
            struct Check<T: ?Sized>(T);
            trait AmbiguousIfImpl<A> { fn some_item() { } }

            impl<T: ?Sized> AmbiguousIfImpl<()> for Check<T> { }
            impl<T: ?Sized $(+ $t)*> AmbiguousIfImpl<u8> for Check<T> { }

            <Check::<$x> as AmbiguousIfImpl<_>>::some_item()
        };
    };
}

macro_rules! send_and_sync_impl {
    ($t:ty) => {
        is_send_sync::<async_watch2::Sender<$t>>();
        assert_not_impl!(async_watch2::Sender<$t>, Clone);

        is_send_sync::<async_watch2::Receiver<$t>>();
        is_clone::<async_watch2::Receiver<$t>>();
    };
}

async_assert_fn!(async_watch2::Sender<u8>::closed(_): Send & Sync);
async_assert_fn!(async_watch2::Sender<Cell<u8>>::closed(_): !Send & !Sync);
async_assert_fn!(async_watch2::Sender<Rc<u8>>::closed(_): !Send & !Sync);

#[test]
fn send_and_sync_clone() {
    send_and_sync_impl!(usize);
    send_and_sync_impl!(Box<usize>);
}
