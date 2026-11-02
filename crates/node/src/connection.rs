use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use reqwest::header::{CONNECTION, UPGRADE};
use reqwest::{Client, StatusCode, Upgraded, Url};
use serde::Deserialize;
use tsoup_proto::node::PROTOCOL;
use tsoup_workspace::rpc;

use crate::node::Node;

const FIRST_RETRY: Duration = Duration::from_secs(1);
const LAST_RETRY: Duration = Duration::from_secs(30);

#[derive(Deserialize)]
struct ErrorBody {
    error: String,
}

enum Refusal {
    Fatal(anyhow::Error),
    Retry(anyhow::Error),
}

pub async fn connect(url: &Url, node: &Arc<Node>) -> anyhow::Result<()> {
    let client = Client::new();
    let mut retry = FIRST_RETRY;

    loop {
        match upgrade(&client, url).await {
            Ok(server) => {
                tracing::info!(%url, "connected to the server");
                retry = FIRST_RETRY;

                match rpc::serve(server, Arc::clone(node)).await {
                    Ok(()) => tracing::warn!("the server closed the connection"),
                    Err(error) => tracing::warn!("lost the server: {error:#}"),
                }
            },
            Err(Refusal::Fatal(error)) => return Err(error),
            Err(Refusal::Retry(error)) => tracing::warn!(
                retry_secs = retry.as_secs(),
                "could not connect to the server: {error:#}"
            ),
        }

        tokio::time::sleep(retry).await;
        retry = (retry * 2).min(LAST_RETRY);
    }
}

async fn upgrade(client: &Client, url: &Url) -> Result<Upgraded, Refusal> {
    let response = client
        .get(url.clone())
        .header(CONNECTION, "upgrade")
        .header(UPGRADE, PROTOCOL)
        .send()
        .await
        .map_err(|error| Refusal::Retry(error.into()))?;
    let status = response.status();

    if status != StatusCode::SWITCHING_PROTOCOLS {
        let message = response
            .json::<ErrorBody>()
            .await
            .map_or_else(|_| status.to_string(), |body| body.error);
        let error = anyhow!("the server refused the node: {message}");

        return Err(
            if status.is_client_error() && status != StatusCode::CONFLICT {
                Refusal::Fatal(error)
            } else {
                Refusal::Retry(error)
            },
        );
    }

    response
        .upgrade()
        .await
        .map_err(|error| Refusal::Retry(error.into()))
}
