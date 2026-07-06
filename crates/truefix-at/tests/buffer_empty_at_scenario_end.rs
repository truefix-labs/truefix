use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use truefix_at::runner::{ExpectMsg, Scenario, SessionTweaks, Step, client_message, run_scenario};
use truefix_core::Field;

/// A minimal fake server: reads (and discards) one inbound frame, replies with a Logon ack, then
/// sends one extra, unsolicited Heartbeat the scenario never asked for.
async fn spawn_server_with_spurious_extra_message() -> std::net::SocketAddr {
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

        let extra = client_message("FIX.4.4", "0", 2); // spurious, unrequested Heartbeat
        stream.write_all(&extra.encode()).await.unwrap();
    });
    addr
}

fn logon_only_scenario() -> Scenario {
    let mut logon = client_message("FIX.4.4", "A", 1);
    logon.body.set(Field::int(98, 0));
    logon.body.set(Field::int(108, 30));
    Scenario {
        name: "test_LogonOnly".to_owned(),
        versions: vec!["FIX.4.4".to_owned()],
        steps: vec![Step::Send(logon), Step::Expect(ExpectMsg::of("A"))],
        tweaks: SessionTweaks::default(),
    }
}

#[tokio::test]
async fn run_scenario_fails_when_a_spurious_extra_message_remains_buffered() {
    let addr = spawn_server_with_spurious_extra_message().await;
    let result = run_scenario(&logon_only_scenario(), addr).await;
    assert!(
        result.is_err(),
        "expected run_scenario to fail on the extra unrequested message, got {result:?}"
    );
}
