use std::time::Duration;

use anyhow::anyhow;
use reqwest::header::{CONNECTION, UPGRADE};
use reqwest::{Client, StatusCode, Upgraded, Url};
use serde::Deserialize;

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

pub async fn keep<F>(url: &Url, protocol: &'static str, mut serve: F) -> anyhow::Result<()>
where
    F: AsyncFnMut(Upgraded) -> anyhow::Result<()>,
{
    let client = Client::new();
    let mut retry = FIRST_RETRY;

    loop {
        match upgrade(&client, url, protocol).await {
            Ok(server) => {
                tracing::info!(%url, "connected to the server");
                retry = FIRST_RETRY;

                match serve(server).await {
                    Ok(()) => tracing::warn!(%url, "the server closed the connection"),
                    Err(error) => tracing::warn!(%url, "lost the server: {error:#}"),
                }
            },
            Err(Refusal::Fatal(error)) => return Err(error),
            Err(Refusal::Retry(error)) => tracing::warn!(
                %url,
                retry_secs = retry.as_secs(),
                "could not connect to the server: {error:#}"
            ),
        }

        tokio::time::sleep(retry).await;
        retry = (retry * 2).min(LAST_RETRY);
    }
}

async fn upgrade(client: &Client, url: &Url, protocol: &'static str) -> Result<Upgraded, Refusal> {
    let response = client
        .get(url.clone())
        .header(CONNECTION, "upgrade")
        .header(UPGRADE, protocol)
        .send()
        .await
        .map_err(|error| Refusal::Retry(error.into()))?;
    let status = response.status();

    if status != StatusCode::SWITCHING_PROTOCOLS {
        let message = response
            .json::<ErrorBody>()
            .await
            .map_or_else(|_| status.to_string(), |body| body.error);
        let error = anyhow!("the server refused the connection: {message}");

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
