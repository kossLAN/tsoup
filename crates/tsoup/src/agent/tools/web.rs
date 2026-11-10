use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::header::CONTENT_TYPE;
use reqwest::redirect::Policy;
use reqwest::{Client, Url};
use scraper::{ElementRef, Html, Selector};

use super::ToolError;

const USER_AGENT: &str = concat!("tsoup/", env!("CARGO_PKG_VERSION"));
const TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REDIRECTS: usize = 5;
const MAX_BODY: usize = 4 * 1024 * 1024;
const CONTENT: &str = "main, article, [role=main]";
const SKIPPED: [&str; 12] = [
    "head", "script", "style", "noscript", "template", "svg", "nav", "footer", "aside", "form",
    "iframe", "button",
];
const BLOCKS: [&str; 20] = [
    "p",
    "div",
    "section",
    "article",
    "main",
    "header",
    "ul",
    "ol",
    "li",
    "table",
    "tr",
    "pre",
    "blockquote",
    "br",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
];

pub struct Page {
    pub url: Url,
    pub html: bool,
    pub body: String,
}

struct PublicResolver;

impl Resolve for PublicResolver {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let addresses: Vec<SocketAddr> = tokio::net::lookup_host((name.as_str(), 0))
                .await?
                .filter(|address| public(address.ip()))
                .collect();

            if addresses.is_empty() {
                return Err(ToolError::Private.into());
            }

            Ok(Box::new(addresses.into_iter()) as Addrs)
        })
    }
}

pub fn client() -> Client {
    let redirects = Policy::custom(|attempt| {
        if attempt.previous().len() > MAX_REDIRECTS {
            attempt.error("too many redirects")
        } else if let Err(error) = check(attempt.url()) {
            attempt.error(error)
        } else {
            attempt.follow()
        }
    });

    Client::builder()
        .user_agent(USER_AGENT)
        .timeout(TIMEOUT)
        .redirect(redirects)
        .dns_resolver(Arc::new(PublicResolver))
        .build()
        .expect("the web client is built from constants")
}

pub async fn get(http: &Client, url: Url) -> Result<Page, ToolError> {
    check(&url)?;

    let mut response = http.get(url).send().await?.error_for_status()?;
    let kind = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|kind| kind.to_str().ok())
        .unwrap_or("text/html")
        .to_lowercase();
    let html = kind.contains("html");

    if !html && !kind.starts_with("text/") && !kind.contains("json") && !kind.contains("xml") {
        return Err(ToolError::NotText(kind));
    }

    let mut body = Vec::new();

    while let Some(chunk) = response.chunk().await? {
        body.extend_from_slice(&chunk);

        if body.len() >= MAX_BODY {
            break;
        }
    }

    Ok(Page {
        url: response.url().clone(),
        html,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

pub fn text(page: &Page) -> String {
    let document = Html::parse_document(&page.body);
    let content = Selector::parse(CONTENT).expect("the content selector is valid");
    let root = document
        .select(&content)
        .next()
        .unwrap_or_else(|| document.root_element());
    let mut raw = String::new();
    render(root, &page.url, false, &mut raw);

    let mut text = String::new();
    let mut blank = true;

    for line in raw.lines().map(str::trim_end) {
        if line.trim().is_empty() {
            if !blank {
                text.push('\n');
            }

            blank = true;
        } else {
            text.push_str(line);
            text.push('\n');
            blank = false;
        }
    }

    text.trim_end().to_owned()
}

fn render(element: ElementRef<'_>, base: &Url, verbatim: bool, text: &mut String) {
    let name = element.value().name();

    if SKIPPED.contains(&name) {
        return;
    }

    let block = BLOCKS.contains(&name);
    let verbatim = verbatim || name == "pre";
    let link = (name == "a")
        .then(|| element.value().attr("href"))
        .flatten()
        .and_then(|href| base.join(href).ok())
        .filter(|url| matches!(url.scheme(), "http" | "https"));

    if block && !text.ends_with('\n') {
        text.push('\n');
    }

    match name {
        "li" => text.push_str("- "),
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => text.push_str("\n# "),
        _ => {},
    }

    if link.is_some() {
        text.push('[');
    }

    for child in element.children() {
        if let Some(content) = child.value().as_text() {
            if verbatim {
                text.push_str(content);
            } else {
                squeeze(content, text);
            }
        } else if let Some(child) = ElementRef::wrap(child) {
            render(child, base, verbatim, text);
        }
    }

    if let Some(url) = link {
        text.push_str(&format!("]({url})"));
    }

    if block && !text.ends_with('\n') {
        text.push('\n');
    }
}

fn squeeze(content: &str, text: &mut String) {
    let mut spaced = text.ends_with(char::is_whitespace) || text.is_empty();

    for character in content.chars() {
        if !character.is_whitespace() {
            text.push(character);
            spaced = false;
        } else if !spaced {
            text.push(' ');
            spaced = true;
        }
    }
}

fn check(url: &Url) -> Result<(), ToolError> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ToolError::Url(url.to_string()));
    }

    let literal = url
        .host_str()
        .and_then(|host| host.trim_matches(['[', ']']).parse::<IpAddr>().ok());

    if literal.is_some_and(|address| !public(address)) {
        return Err(ToolError::Private);
    }

    Ok(())
}

fn public(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => public_v4(address),
        IpAddr::V6(address) => address
            .to_ipv4_mapped()
            .map_or_else(|| public_v6(address), public_v4),
    }
}

fn public_v4(address: Ipv4Addr) -> bool {
    let [first, second, ..] = address.octets();

    !(address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        || address.is_unspecified()
        || address.is_broadcast()
        || address.is_documentation()
        || address.is_multicast()
        || (first == 100 && (64..128).contains(&second)))
}

fn public_v6(address: Ipv6Addr) -> bool {
    let [first, ..] = address.segments();

    !(address.is_loopback()
        || address.is_unspecified()
        || address.is_multicast()
        || first & 0xfe00 == 0xfc00
        || first & 0xffc0 == 0xfe80)
}
