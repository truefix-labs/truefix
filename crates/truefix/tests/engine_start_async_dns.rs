//! T073 — proxy hostname resolution in `Engine::start` must yield on a current-thread runtime.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, Waker};

use truefix::config::SessionSettings;
use truefix::{Application, Engine};

struct NoopApp;

#[async_trait::async_trait]
impl Application for NoopApp {}

#[tokio::test(flavor = "current_thread")]
async fn proxy_dns_yields_to_the_current_thread_executor() {
    let settings = SessionSettings::parse(
        "[DEFAULT]\n\
         BeginString=FIX.4.4\n\
         HeartBtInt=30\n\
         \n\
         [SESSION]\n\
         ConnectionType=initiator\n\
         SenderCompID=CLIENT\n\
         TargetCompID=SERVER\n\
         SocketConnectHost=127.0.0.1\n\
         SocketConnectPort=9\n\
         ProxyType=HttpConnect\n\
         ProxyHost=proxy-does-not-exist.invalid\n\
         ProxyPort=8080\n",
    )
    .expect("parse test settings");

    let peer_ran = Arc::new(AtomicBool::new(false));
    let peer_flag = peer_ran.clone();
    let peer = tokio::spawn(async move {
        tokio::task::yield_now().await;
        peer_flag.store(true, Ordering::SeqCst);
    });

    let mut start = Box::pin(Engine::start(&settings, Arc::new(NoopApp)));
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);

    assert!(
        matches!(start.as_mut().poll(&mut context), Poll::Pending),
        "Engine::start completed its proxy DNS lookup in its first poll; a blocking resolver can \
         stall every other task on a current-thread runtime"
    );

    peer.await.expect("peer task completes");
    assert!(
        peer_ran.load(Ordering::SeqCst),
        "the current-thread executor should remain available while proxy DNS is pending"
    );
}
