use reqwest::{Client, Url};
use schemars::JsonSchema;
use scraper::{ElementRef, Html, Selector};
use serde::Deserialize;

use super::{Caller, Outcome, Tool, ToolError, web};
use crate::workspace::Workspace;

const ENDPOINT: &str = "https://html.duckduckgo.com/html/";
const REDIRECT: &str = "/l/";
const TARGET: &str = "uddg";
const MAX_RESULTS: usize = 10;

pub struct WebSearch(pub Client);

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(description = "What to search for")]
    query: String,
}

impl<E: Workspace> Tool<E> for WebSearch {
    const NAME: &'static str = "web_search";
    const DESCRIPTION: &'static str = "Search the web with DuckDuckGo.
Returns the title, address and a snippet of each result, read a result with web_fetch.";

    type Arguments = Arguments;

    async fn run(
        &self,
        _caller: &Caller<'_>,
        _environment: &E,
        Arguments { query }: Arguments,
    ) -> Result<Outcome, ToolError> {
        let url =
            Url::parse_with_params(ENDPOINT, [("q", &query)]).map_err(|_| ToolError::Url(query))?;
        let page = web::get(&self.0, url).await?;
        let results = results(&page.body);

        if results.is_empty() {
            return Err(ToolError::NoResults);
        }

        Ok(Outcome::text(results.join("\n\n")))
    }
}

fn results(html: &str) -> Vec<String> {
    let document = Html::parse_document(html);
    let result = Selector::parse(".result:not(.result--ad)").expect("the selector is valid");
    let title = Selector::parse(".result__a").expect("the selector is valid");
    let snippet = Selector::parse(".result__snippet").expect("the selector is valid");

    document
        .select(&result)
        .filter_map(|result| {
            let link = result.select(&title).next()?;
            let address = target(link.value().attr("href")?)?;
            let snippet = result
                .select(&snippet)
                .next()
                .map(words)
                .unwrap_or_default();
            Some(format!("{}\n{address}\n{snippet}", words(link)))
        })
        .take(MAX_RESULTS)
        .collect()
}

fn target(href: &str) -> Option<String> {
    let url = Url::parse(ENDPOINT).ok()?.join(href).ok()?;

    if url.path() != REDIRECT {
        return Some(url.into());
    }

    url.query_pairs()
        .find(|(name, _)| name == TARGET)
        .map(|(_, target)| target.into_owned())
}

fn words(element: ElementRef<'_>) -> String {
    element
        .text()
        .flat_map(str::split_whitespace)
        .collect::<Vec<_>>()
        .join(" ")
}
