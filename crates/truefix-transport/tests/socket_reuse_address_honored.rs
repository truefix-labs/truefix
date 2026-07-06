use std::sync::Arc;

use truefix_session::Application;
use truefix_transport::{AcceptorBuilder, Services, SocketOptions};

struct NoopApp;

#[async_trait::async_trait]
impl Application for NoopApp {}

#[tokio::test]
async fn socket_reuse_address_false_is_applied_before_acceptor_builder_binds() {
    let acceptor = AcceptorBuilder::bind_with(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(NoopApp),
        Services {
            socket_options: SocketOptions {
                reuse_address: false,
                ..SocketOptions::default()
            },
            ..Services::default()
        },
    )
    .await
    .unwrap();

    assert!(!acceptor.listener_reuse_address().unwrap());
}
