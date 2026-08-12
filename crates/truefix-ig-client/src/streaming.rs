//! IG Lightstreamer integration built on the TLCP protocol client.

use lightstreamer_rs::{
    Client, ClientConfig, Credentials, FieldSchema, ItemGroup, ServerAddress, Snapshot,
    Subscription, SubscriptionMode, Updates,
};

use crate::error::IgResult;

// Keep the top-of-book subscription to IG's documented portable field set.
// Optional fields differ by account and instrument and IG rejects the entire
// subscription when even one requested field is unavailable.
const MARKET_FIELDS: [&str; 5] = [
    "BID",
    "OFFER",
    "UPDATE_TIME",
    "MARKET_STATE",
    "MARKET_DELAY",
];
const MARKET_LADDER_FIELDS: [&str; 20] = [
    "BIDPRICE1",
    "BIDPRICE2",
    "BIDPRICE3",
    "BIDPRICE4",
    "BIDPRICE5",
    "ASKPRICE1",
    "ASKPRICE2",
    "ASKPRICE3",
    "ASKPRICE4",
    "ASKPRICE5",
    "BIDSIZE1",
    "BIDSIZE2",
    "BIDSIZE3",
    "BIDSIZE4",
    "BIDSIZE5",
    "ASKSIZE1",
    "ASKSIZE2",
    "ASKSIZE3",
    "ASKSIZE4",
    "ASKSIZE5",
];
const ACCOUNT_FIELDS: [&str; 5] = ["PNL", "DEPOSIT", "USED_MARGIN", "AVAILABLE_CASH", "FUNDS"];
const TRADE_FIELDS: [&str; 3] = ["CONFIRMS", "OPU", "WOU"];

fn market_item(epic: &str) -> String {
    format!("MARKET:{epic}")
}

fn market_ladder_item(account_id: &str, epic: &str) -> String {
    format!("PRICE:{account_id}:{epic}")
}

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
        let items = epics.into_iter().map(|epic| market_item(epic.as_ref()));
        self.subscribe(SubscriptionMode::Merge, items, MARKET_FIELDS, Snapshot::On)
            .await
    }

    /// Subscribes to IG's optional native five-level price ladder.
    ///
    /// Ladder availability is account- and instrument-dependent. Keeping this
    /// subscription separate prevents an unsupported ladder field set from
    /// rejecting the ordinary top-of-book market subscription.
    pub async fn subscribe_market_ladders<I, S>(&self, epics: I) -> IgResult<Updates>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let items = epics
            .into_iter()
            .map(|epic| market_ladder_item(&self.account_id, epic.as_ref()));
        self.subscribe(
            SubscriptionMode::Merge,
            items,
            MARKET_LADDER_FIELDS,
            Snapshot::On,
        )
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

#[cfg(test)]
mod tests {
    use super::{market_item, market_ladder_item};

    #[test]
    fn market_and_ladder_use_distinct_official_item_names() {
        assert_eq!(market_item("IX.D.DAX.IFD.IP"), "MARKET:IX.D.DAX.IFD.IP");
        assert_eq!(
            market_ladder_item("ABC123", "IX.D.DAX.IFD.IP"),
            "PRICE:ABC123:IX.D.DAX.IFD.IP"
        );
    }
}
