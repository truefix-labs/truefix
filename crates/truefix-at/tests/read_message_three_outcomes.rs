use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use truefix_at::runner::{ReadMessageOutcome, read_message};
use truefix_core::{Field, Message};

async fn stream_pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (client, accepted) = tokio::join!(TcpStream::connect(address), listener.accept());
    (client.unwrap(), accepted.unwrap().0)
}

fn framed_but_undecodable_message() -> Vec<u8> {
    let mut message = Message::new();
    message.header.set(Field::string(8, "FIX.4.4"));
    message.header.set(Field::string(35, "0"));
    let mut bytes = message.encode();
    let checksum_digit = bytes.len() - 2;
    bytes[checksum_digit] = if bytes[checksum_digit] == b'9' {
        b'0'
    } else {
        b'9'
    };
    bytes
}

#[tokio::test]
async fn timeout_decode_failure_and_clean_eof_are_distinct_outcomes() {
    let (mut client, server) = stream_pair().await;
    let mut buffer = Vec::new();
    let timed_out = read_message(&mut client, &mut buffer, Duration::from_millis(20)).await;

    drop(server);
    let clean_eof = read_message(&mut client, &mut buffer, Duration::from_secs(1)).await;

    let (mut client, mut server) = stream_pair().await;
    server
        .write_all(&framed_but_undecodable_message())
        .await
        .unwrap();
    let decode_failed = read_message(&mut client, &mut Vec::new(), Duration::from_secs(1)).await;

    assert!(matches!(timed_out, ReadMessageOutcome::TimedOut));
    assert!(matches!(decode_failed, ReadMessageOutcome::DecodeFailed(_)));
    assert!(matches!(clean_eof, ReadMessageOutcome::CleanEof));
}
