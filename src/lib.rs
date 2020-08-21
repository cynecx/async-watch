//! A single-producer, multi-consumer channel that only retains the *last* sent
//! value.
//!
//! Extracted from [Tokio's](https://github.com/tokio-rs/tokio/) `tokio::sync::watch`
//! implementation, which was initially written by [Carl Lerche](https://github.com/carllerche).
//!
//! This channel is useful for watching for changes to a value from multiple
//! points in the code base, for example, changes to configuration values.
//!
//! # Usage
//!
//! [`channel`] returns a [`Sender`] / [`Receiver`] pair. These are
//! the producer and sender halves of the channel. The channel is
//! created with an initial value. [`Receiver::recv`] will always
//! be ready upon creation and will yield either this initial value or
//! the latest value that has been sent by `Sender`.
//!
//! Calls to [`Receiver::recv`] will always yield the latest value.
//!
//! # Examples
//!
//! ```
//! # let executor = async_executor::LocalExecutor::new();
//! # executor.run(async {
//! let (tx, mut rx) = async_watch::channel("hello");
//!
//! executor.spawn(async move {
//!     while let Some(value) = rx.recv().await {
//!         println!("received = {:?}", value);
//!     }
//! });
//!
//! tx.broadcast("world").unwrap();
//! # });
//! ```
//!
//! # Closing
//!
//! [`Sender::closed`] allows the producer to detect when all [`Receiver`]
//! handles have been dropped. This indicates that there is no further interest
//! in the values being produced and work can be stopped.
//!
//! # Thread safety
//!
//! Both [`Sender`] and [`Receiver`] are thread safe. They can be moved to other
//! threads and can be used in a concurrent environment. Clones of [`Receiver`]
//! handles may be moved to separate threads and also used concurrently.
//!
//! [`Sender`]: crate::Sender
//! [`Receiver`]: crate::Receiver
//! [`Receiver::recv`]: crate::Receiver::recv
//! [`channel`]: crate::channel
//! [`Sender::closed`]: crate::Sender::closed

pub mod error;

mod poll_fn;
use poll_fn::poll_fn;

use atomic_waker::AtomicWaker;
use fnv::FnvHashSet;

use std::ops;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::{Relaxed, SeqCst};
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, Weak};
use std::task::Poll::{Pending, Ready};
use std::task::{Context, Poll};

/// The initial version starts at zero.
const VERSION_0: usize = 0b00;

/// The version counter shifted by one position to the left to leave space for the closed bit.
const VERSION_1: usize = 0b10;

/// The least significant bit signifies a closed channel.
const CLOSED: usize = 0b01;

/// Receives values from the associated [`Sender`](struct@Sender).
///
/// Instances are created by the [`channel`](fn@channel) function.
#[derive(Debug)]
pub struct Receiver<T> {
    /// Pointer to the shared state
    shared: Arc<Shared<T>>,

    /// Pointer to the watcher's internal state
    inner: Watcher,
}

/// Sends values to the associated [`Receiver`](struct@Receiver).
///
/// Instances are created by the [`channel`](fn@channel) function.
#[derive(Debug)]
pub struct Sender<T> {
    shared: Weak<Shared<T>>,
}

/// Returns a reference to the inner value
///
/// Outstanding borrows hold a read lock on the inner value. This means that
/// long lived borrows could cause the produce half to block. It is recommended
/// to keep the borrow as short lived as possible.
#[derive(Debug)]
pub struct Ref<'a, T> {
    inner: RwLockReadGuard<'a, T>,
}

#[derive(Debug)]
struct Shared<T> {
    /// The most recent value
    value: RwLock<T>,

    /// The current version
    ///
    /// The lowest bit represents a "closed" state. The rest of the bits
    /// represent the current version.
    version: AtomicUsize,

    /// All watchers
    watchers: Mutex<Watchers>,

    /// Task to notify when all watchers drop
    cancel: AtomicWaker,
}

type Watchers = FnvHashSet<Watcher>;

/// The watcher's ID is based on the Arc's pointer.
#[derive(Clone, Debug)]
struct Watcher(Arc<WatchInner>);

#[derive(Debug)]
struct WatchInner {
    /// Last observed version
    version: AtomicUsize,
    waker: AtomicWaker,
}

/// Creates a new watch channel, returning the "send" and "receive" handles.
///
/// All values sent by [`Sender`] will become visible to the [`Receiver`] handles.
/// Only the last value sent is made available to the [`Receiver`] half. All
/// intermediate values are dropped.
///
/// # Examples
///
/// ```
/// # let executor = async_executor::LocalExecutor::new();
/// # executor.run(async {
/// let (tx, mut rx) = async_watch::channel("hello");
///
/// executor.spawn(async move {
///     while let Some(value) = rx.recv().await {
///         println!("received = {:?}", value);
///     }
/// });
///
/// tx.broadcast("world").unwrap();
/// # });
/// ```
///
/// [`Sender`]: struct@Sender
/// [`Receiver`]: struct@Receiver
pub fn channel<T: Clone>(init: T) -> (Sender<T>, Receiver<T>) {
    // We don't start knowing VERSION_1
    let inner = Watcher::new_version(VERSION_0);

    // Insert the watcher
    let mut watchers = Watchers::with_capacity_and_hasher(0, Default::default());
    watchers.insert(inner.clone());

    let shared = Arc::new(Shared {
        value: RwLock::new(init),
        version: AtomicUsize::new(VERSION_1),
        watchers: Mutex::new(watchers),
        cancel: AtomicWaker::new(),
    });

    let tx = Sender {
        shared: Arc::downgrade(&shared),
    };

    let rx = Receiver { shared, inner };

    (tx, rx)
}

impl<T> Receiver<T> {
    /// Returns a reference to the most recently sent value
    ///
    /// Outstanding borrows hold a read lock. This means that long lived borrows
    /// could cause the send half to block. It is recommended to keep the borrow
    /// as short lived as possible.
    ///
    /// # Examples
    ///
    /// ```
    /// let (_, rx) = async_watch::channel("hello");
    /// assert_eq!(*rx.borrow(), "hello");
    /// ```
    pub fn borrow(&self) -> Ref<'_, T> {
        let inner = self.shared.value.read().unwrap();
        Ref { inner }
    }

    // TODO: document
    #[doc(hidden)]
    pub fn poll_recv_ref<'a>(&'a mut self, cx: &mut Context<'_>) -> Poll<Option<Ref<'a, T>>> {
        // Make sure the task is up to date
        self.inner.waker.register(cx.waker());

        let state = self.shared.version.load(SeqCst);
        let version = state & !CLOSED;

        if self.inner.version.swap(version, Relaxed) != version {
            let inner = self.shared.value.read().unwrap();

            return Ready(Some(Ref { inner }));
        }

        if CLOSED == state & CLOSED {
            // The `Store` handle has been dropped.
            return Ready(None);
        }

        Pending
    }
}

impl<T: Clone> Receiver<T> {
    /// Attempts to clone the latest value sent via the channel.
    ///
    /// If this is the first time the function is called on a `Receiver`
    /// instance, then the function completes immediately with the **current**
    /// value held by the channel. On the next call, the function waits until
    /// a new value is sent in the channel.
    ///
    /// `None` is returned if the `Sender` half is dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// # let executor = async_executor::LocalExecutor::new();
    /// # executor.run(async {
    /// let (tx, mut rx) = async_watch::channel("hello");
    ///
    /// let v = rx.recv().await.unwrap();
    /// assert_eq!(v, "hello");
    ///
    /// let task = executor.spawn(async move {
    ///     tx.broadcast("goodbye").unwrap();
    /// });
    ///
    /// // Waits for the new task to spawn and send the value.
    /// let v = rx.recv().await.unwrap();
    /// assert_eq!(v, "goodbye");
    ///
    /// let v = rx.recv().await;
    /// assert!(v.is_none());
    ///
    /// task.await;
    /// # });
    /// ```
    pub async fn recv(&mut self) -> Option<T> {
        poll_fn(|cx| {
            let v_ref = match self.poll_recv_ref(cx) {
                Ready(v) => v,
                Pending => return Pending,
            };
            Poll::Ready(v_ref.map(|v_ref| (*v_ref).clone()))
        })
        .await
    }
}

impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        let ver = self.inner.version.load(Relaxed);
        let inner = Watcher::new_version(ver);
        let shared = self.shared.clone();

        shared.watchers.lock().unwrap().insert(inner.clone());

        Receiver { shared, inner }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.shared.watchers.lock().unwrap().remove(&self.inner);
    }
}

impl<T> Sender<T> {
    /// Broadcasts a new value via the channel, notifying all receivers.
    pub fn broadcast(&self, value: T) -> Result<(), error::SendError<T>> {
        let shared = match self.shared.upgrade() {
            Some(shared) => shared,
            // All `Watch` handles have been canceled
            None => return Err(error::SendError { inner: value }),
        };

        // Replace the value
        {
            let mut lock = shared.value.write().unwrap();
            *lock = value;
        }

        // Update the version. 2 (`VERSION_1`) is used so that the CLOSED bit is not set.
        shared.version.fetch_add(VERSION_1, SeqCst);

        // Notify all watchers
        notify_all(&*shared);

        Ok(())
    }

    /// Completes when all receivers have dropped.
    ///
    /// This allows the producer to get notified when interest in the produced
    /// values is canceled and immediately stop doing work.
    pub async fn closed(&mut self) {
        poll_fn(|cx| self.poll_close(cx)).await
    }

    fn poll_close(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        match self.shared.upgrade() {
            Some(shared) => {
                shared.cancel.register(cx.waker());
                Pending
            }
            None => Ready(()),
        }
    }
}

/// Notifies all watchers of a change
fn notify_all<T>(shared: &Shared<T>) {
    let watchers = shared.watchers.lock().unwrap();

    for watcher in watchers.iter() {
        // Notify the task
        watcher.waker.wake();
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        if let Some(shared) = self.shared.upgrade() {
            shared.version.fetch_or(CLOSED, SeqCst);
            notify_all(&*shared);
        }
    }
}

// ===== impl Ref =====

impl<T> ops::Deref for Ref<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.inner.deref()
    }
}

// ===== impl Shared =====

impl<T> Drop for Shared<T> {
    fn drop(&mut self) {
        self.cancel.wake();
    }
}

// ===== impl Watcher =====

impl Watcher {
    fn new_version(version: usize) -> Self {
        Watcher(Arc::new(WatchInner {
            version: AtomicUsize::new(version),
            waker: AtomicWaker::new(),
        }))
    }
}

impl std::cmp::PartialEq for Watcher {
    fn eq(&self, other: &Watcher) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::cmp::Eq for Watcher {}

impl std::hash::Hash for Watcher {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (&*self.0 as *const WatchInner).hash(state)
    }
}

impl std::ops::Deref for Watcher {
    type Target = WatchInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
