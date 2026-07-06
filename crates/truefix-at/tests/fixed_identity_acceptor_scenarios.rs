use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use truefix_at::runner::{
    ReadMessageOutcome, SessionTweaks, client_message, read_message, start_fixed_identity_acceptor,
};
use truefix_core::Field;

fn logon(begin_string: &str, sender: &str, target: &str, seq: i64) -> truefix_core::Message {
    let mut m = client_message(begin_string, "A", seq);
    m.header.set(Field::string(49, sender));
    m.header.set(Field::string(56, target));
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    m
}

#[tokio::test]
async fn a_logon_matching_the_fixed_identity_is_accepted() {
    let (addr, handle) =
        start_fixed_identity_acceptor("FIX.4.4", "SERVER", "CLIENT", &SessionTweaks::default())
            .await
            .unwrap();
    let mut stream = TcpStream::connect(addr).await.unwrap();
    // The client's CompIDs are the mirror of the acceptor's own configured identity.
    stream
        .write_all(&logon("FIX.4.4", "CLIENT", "SERVER", 1).encode())
        .await
        .unwrap();
    let mut buf = Vec::new();
    match read_message(&mut stream, &mut buf, Duration::from_secs(3)).await {
        ReadMessageOutcome::Message(msg) => assert_eq!(msg.msg_type(), Some("A")),
        other => panic!("expected a Logon ack, got {other:?}"),
    }
    handle.abort();
}

#[tokio::test]
async fn a_logon_with_the_wrong_sender_comp_id_is_refused() {
    let (addr, handle) =
        start_fixed_identity_acceptor("FIX.4.4", "SERVER", "CLIENT", &SessionTweaks::default())
            .await
            .unwrap();
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(&logon("FIX.4.4", "WRONG-SENDER", "SERVER", 1).encode())
        .await
        .unwrap();
    let mut buf = Vec::new();
    match read_message(&mut stream, &mut buf, Duration::from_secs(3)).await {
        ReadMessageOutcome::CleanEof | ReadMessageOutcome::ReadFailed(_) => {}
        other => panic!("expected the mismatched connection to be refused, got {other:?}"),
    }
    handle.abort();
}

#[tokio::test]
async fn a_logon_with_the_wrong_target_comp_id_is_refused() {
    let (addr, handle) =
        start_fixed_identity_acceptor("FIX.4.4", "SERVER", "CLIENT", &SessionTweaks::default())
            .await
            .unwrap();
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(&logon("FIX.4.4", "CLIENT", "WRONG-TARGET", 1).encode())
        .await
        .unwrap();
    let mut buf = Vec::new();
    match read_message(&mut stream, &mut buf, Duration::from_secs(3)).await {
        ReadMessageOutcome::CleanEof | ReadMessageOutcome::ReadFailed(_) => {}
        other => panic!("expected the mismatched connection to be refused, got {other:?}"),
    }
    handle.abort();
}

#[tokio::test]
async fn a_logon_with_the_wrong_begin_string_is_refused() {
    let (addr, handle) =
        start_fixed_identity_acceptor("FIX.4.4", "SERVER", "CLIENT", &SessionTweaks::default())
            .await
            .unwrap();
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(&logon("FIX.4.2", "CLIENT", "SERVER", 1).encode())
        .await
        .unwrap();
    let mut buf = Vec::new();
    match read_message(&mut stream, &mut buf, Duration::from_secs(3)).await {
        ReadMessageOutcome::CleanEof | ReadMessageOutcome::ReadFailed(_) => {}
        other => panic!("expected the mismatched connection to be refused, got {other:?}"),
    }
    handle.abort();
}
