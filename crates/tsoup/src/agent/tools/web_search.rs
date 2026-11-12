use reqwest::{Client, StatusCode, Url};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{Caller, Outcome, Scope, Tool, ToolError, web};
use crate::workspace::Workspace;

const NAME: &str = "web_search";
const MAX_RESULTS: usize = 10;

pub struct WebSearch;

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(description = "What to search for")]
    query: String,
}

#[derive(Deserialize)]
struct Answer {
    results: Vec<Hit>,
}

#[derive(Deserialize)]
struct Hit {
    url: Option<String>,
    title: Option<String>,
    content: Option<String>,
}

impl<E: Workspace> Tool<E> for WebSearch {
    const NAME: &'static str = NAME;
    const DESCRIPTION: &'static str = "Search the web.
Returns the title, address and a snippet of each result, read a result with web_fetch.";

    type Arguments = Arguments;

    fn available(scope: Scope) -> bool {
        scope.search
    }

    async fn run(
        &self,
        caller: &Caller<'_>,
        _environment: &E,
        Arguments { query }: Arguments,
    ) -> Result<Outcome, ToolError> {
        let Some(instance) = caller.agent.settings.get().await.search.searxng else {
            return Err(ToolError::Unknown(NAME.to_owned()));
        };
        let results = search(&caller.agent.http, &instance, &query).await?;

        if results.is_empty() {
            return Err(ToolError::NoResults);
        }

        Ok(Outcome::text(results.join("\n\n")))
    }
}

// The instance is one the settings name, so unlike the pages agents ask for it may be private.
async fn search(http: &Client, instance: &str, query: &str) -> Result<Vec<String>, ToolError> {
    let url = Url::parse_with_params(
        &format!("{instance}/search"),
        [("q", query), ("format", "json")],
    )
    .map_err(|_| ToolError::Url(instance.to_owned()))?;
    let response = http.get(url).timeout(web::TIMEOUT).send().await?;

    // SearXNG forbids every format its settings do not list, which is all but html by default.
    if response.status() == StatusCode::FORBIDDEN {
        return Err(ToolError::SearchFormat);
    }

    let answer: Answer = response.error_for_status()?.json().await?;

    Ok(answer
        .results
        .into_iter()
        .filter_map(|hit| {
            let snippet = hit.content.unwrap_or_default();
            Some(format!(
                "{}\n{}\n{}",
                words(&hit.title?),
                hit.url?,
                words(&snippet)
            ))
        })
        .take(MAX_RESULTS)
        .collect())
}

fn words(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
