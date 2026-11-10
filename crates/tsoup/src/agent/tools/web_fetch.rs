use reqwest::{Client, Url};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{Caller, Outcome, Tool, ToolError, head, web};
use crate::workspace::Workspace;

pub struct WebFetch(pub Client);

#[derive(Deserialize, JsonSchema)]
pub struct Arguments {
    #[schemars(description = "The http or https address of the page")]
    url: String,
}

impl<E: Workspace> Tool<E> for WebFetch {
    const NAME: &'static str = "web_fetch";
    const DESCRIPTION: &'static str = "Fetch a web page and read its text.
Links are kept as [text](address) so you can follow them with another call.";

    type Arguments = Arguments;

    async fn run(
        &self,
        _caller: &Caller<'_>,
        _environment: &E,
        Arguments { url }: Arguments,
    ) -> Result<Outcome, ToolError> {
        let url = Url::parse(&url).map_err(|_| ToolError::Url(url))?;
        let page = web::get(&self.0, url).await?;
        let text = if page.html {
            web::text(&page)
        } else {
            page.body
        };
        Ok(Outcome::text(head(text)))
    }
}
