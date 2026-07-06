use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use truefix_at::runner::{ExpectMsg, Scenario, SessionTweaks, Step, client_message, run_scenario};
use truefix_core::Field;

/// A fake server that answers a Logon with an ack carrying one extra, unrequested field
/// (a made-up tag outside `ALWAYS_ALLOWED_TAGS`).
async fn spawn_server_with_extra_field() -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).await.unwrap();

        let mut ack = client_message("FIX.4.4", "A", 1);
        ack.body.set(Field::int(98, 0));
        ack.body.set(Field::int(108, 30));
        ack.body.set(Field::string(9999, "unexpected")); // extra, unrequested field
        stream.write_all(&ack.encode()).await.unwrap();
    });
    addr
}

async fn spawn_server_without_extra_field() -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).await.unwrap();

        let mut ack = client_message("FIX.4.4", "A", 1);
        ack.body.set(Field::int(98, 0));
        ack.body.set(Field::int(108, 30));
        stream.write_all(&ack.encode()).await.unwrap();
    });
    addr
}

fn logon_scenario(expect: ExpectMsg) -> Scenario {
    let mut logon = client_message("FIX.4.4", "A", 1);
    logon.body.set(Field::int(98, 0));
    logon.body.set(Field::int(108, 30));
    Scenario {
        name: "test_LogonExact".to_owned(),
        versions: vec!["FIX.4.4".to_owned()],
        steps: vec![Step::Send(logon), Step::Expect(expect)],
        tweaks: SessionTweaks::default(),
    }
}

#[tokio::test]
async fn check_match_flags_an_unexpected_extra_field_when_exact() {
    let addr = spawn_server_with_extra_field().await;
    let scenario = logon_scenario(ExpectMsg::of("A").field(98, "0").field(108, "30").exact());
    let result = run_scenario(&scenario, addr).await;
    assert!(
        result.is_err(),
        "expected run_scenario to fail on the unexpected extra field, got {result:?}"
    );
}

#[tokio::test]
async fn check_match_ignores_unlisted_fields_when_not_exact() {
    let addr = spawn_server_with_extra_field().await;
    let scenario = logon_scenario(ExpectMsg::of("A")); // no .exact(), no specific fields
    let result = run_scenario(&scenario, addr).await;
    assert!(
        result.is_ok(),
        "expected non-exact matching to ignore the extra field, got {result:?}"
    );
}

#[tokio::test]
async fn check_match_exact_passes_when_no_extra_field_is_present() {
    let addr = spawn_server_without_extra_field().await;
    let scenario = logon_scenario(ExpectMsg::of("A").field(98, "0").field(108, "30").exact());
    let result = run_scenario(&scenario, addr).await;
    assert!(
        result.is_ok(),
        "expected an exact match to pass: {result:?}"
    );
}
