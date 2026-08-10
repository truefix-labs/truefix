//! IG Lightstreamer integration built on the TLCP protocol client.

use lightstreamer_rs::{
    Client, ClientConfig, Credentials, FieldSchema, ItemGroup, ServerAddress, Snapshot,
    Subscription, SubscriptionMode, Updates,
};

use crate::error::IgResult;

const MARKET_FIELDS: [&str; 11] = [
    "BID",
    "OFFER",
    "HIGH",
    "LOW",
    "MID_OPEN",
    "CHANGE",
    "CHANGE_PCT",
    "UPDATE_TIME",
    "MARKET_STATE",
    "MARKET_DELAY",
    "LTV",
];
const ACCOUNT_FIELDS: [&str; 5] = ["PNL", "DEPOSIT", "USED_MARGIN", "AVAILABLE_CASH", "FUNDS"];
const TRADE_FIELDS: [&str; 3] = ["CONFIRMS", "OPU", "WOU"];

/// An authenticated IG Lightstreamer session.
///
/// Dropping this value disconnects the underlying session and closes its subscriptions.
pub struct IgStreamingClient {
    client: Client,
    account_id: String,
}

impl std::fmt::Debug for IgStreamingClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IgStreamingClient")
            .field("account_id", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl IgStreamingClient {
    pub(crate) async fn connect(
        endpoint: &str,
        account_id: String,
        cst: String,
        x_security_token: String,
    ) -> IgResult<Self> {
        let password = format!("CST-{cst}|XST-{x_security_token}");
        let config = ClientConfig::builder(ServerAddress::try_new(endpoint)?)
            .with_credentials(Credentials::new(&account_id, password))
            .build()?;
        let (client, session_events) = Client::connect(config).await?;
        drop(session_events);
        Ok(Self { client, account_id })
    }

    /// Subscribes to live prices for one or more IG epics.
    pub async fn subscribe_markets<I, S>(&self, epics: I) -> IgResult<Updates>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let items = epics
            .into_iter()
            .map(|epic| format!("MARKET:{}", epic.as_ref()));
        self.subscribe(SubscriptionMode::Merge, items, MARKET_FIELDS, Snapshot::On)
            .await
    }

    /// Subscribes to account balance and margin updates.
    pub async fn subscribe_account(&self) -> IgResult<Updates> {
        self.subscribe(
            SubscriptionMode::Merge,
            [format!("ACCOUNT:{}", self.account_id)],
            ACCOUNT_FIELDS,
            Snapshot::On,
        )
        .await
    }

    /// Subscribes to deal confirmations, position updates, and working-order updates.
    pub async fn subscribe_trades(&self) -> IgResult<Updates> {
        self.subscribe(
            SubscriptionMode::Distinct,
            [format!("TRADE:{}", self.account_id)],
            TRADE_FIELDS,
            Snapshot::On,
        )
        .await
    }

    /// Gracefully closes the Lightstreamer session.
    pub async fn disconnect(self) -> IgResult<()> {
        self.client.disconnect().await?;
        Ok(())
    }

    async fn subscribe<I, S, F>(
        &self,
        mode: SubscriptionMode,
        items: I,
        fields: F,
        snapshot: Snapshot,
    ) -> IgResult<Updates>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
        F: IntoIterator,
        F::Item: Into<String>,
    {
        let subscription = Subscription::new(
            mode,
            ItemGroup::from_items(items)?,
            FieldSchema::from_fields(fields)?,
        )
        .with_snapshot(snapshot);
        Ok(self.client.subscribe(subscription).await?)
    }
}
