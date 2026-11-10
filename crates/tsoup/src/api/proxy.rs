use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::header::{HOST, LOCATION, REFERER, REFRESH};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use axum::middleware::Next;
use axum::response::{IntoResponse as _, Redirect, Response};
use hyper::client::conn::http1;
use hyper::upgrade::OnUpgrade;
use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{ApiError, AppState};

const PREFIX: &str = "/preview/";

pub async fn route(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if let Some(preview) = PreviewPath::parse(request.uri().path()) {
        if preview.rest.is_empty() {
            return redirect(&format!("{}/", preview.base), request.uri());
        }

        return proxy(state, preview, request).await.unwrap_or_else(failed);
    }

    if let Some(preview) = PreviewPath::referring(&request) {
        return redirect(&preview.join(request.uri().path()), request.uri());
    }

    next.run(request).await
}

struct PreviewPath {
    session: Uuid,
    name: String,
    base: String,
    rest: String,
}

impl PreviewPath {
    fn parse(path: &str) -> Option<Self> {
        let (session, rest) = path.strip_prefix(PREFIX)?.split_once('/')?;
        let (name, rest) = rest.split_at(rest.find('/').unwrap_or(rest.len()));

        Some(Self {
            session: session.parse().ok()?,
            name: name.to_owned(),
            base: path[..path.len() - rest.len()].to_owned(),
            rest: rest.to_owned(),
        })
    }

    fn referring(request: &Request) -> Option<Self> {
        let referrer: Uri = request
            .headers()
            .get(REFERER)?
            .to_str()
            .ok()?
            .parse()
            .ok()?;
        Self::parse(referrer.path())
    }

    fn join(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    fn rebase_location(&self, headers: &mut HeaderMap) {
        let location = headers
            .get(LOCATION)
            .and_then(|location| location.to_str().ok())
            .filter(|location| location.starts_with('/') && !location.starts_with("//"))
            .and_then(|location| HeaderValue::try_from(self.join(location)).ok());

        if let Some(location) = location {
            headers.insert(LOCATION, location);
        }
    }
}

async fn proxy(
    state: AppState,
    preview: PreviewPath,
    mut request: Request,
) -> Result<Response, ApiError> {
    let mut sender = connect(&state, &preview).await?;

    *request.uri_mut() = Uri::try_from(with_query(&preview.rest, request.uri()))
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "the preview path is not valid"))?;
    request
        .headers_mut()
        .insert(HOST, HeaderValue::from_static("localhost"));

    let client = hyper::upgrade::on(&mut request);
    let mut response = sender.send_request(request).await.map_err(bad_gateway)?;
    preview.rebase_location(response.headers_mut());

    if response.status() == StatusCode::SWITCHING_PROTOCOLS {
        let vm = hyper::upgrade::on(&mut response);
        tokio::spawn(tunnel(client, vm, state.shutdown.clone()));
    }

    Ok(response.map(Body::new))
}

async fn connect(
    state: &AppState,
    preview: &PreviewPath,
) -> Result<http1::SendRequest<Body>, ApiError> {
    let socket = state
        .session(preview.session)
        .await?
        .sandbox()?
        .vm
        .socket(&preview.name)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "the VM serves no such preview"))?;

    let stream = UnixStream::connect(socket).await.map_err(|_| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "the VM preview is not available",
        )
    })?;

    let (sender, connection) = http1::handshake(TokioIo::new(stream))
        .await
        .map_err(bad_gateway)?;

    tokio::spawn(async move {
        if let Err(error) = connection.with_upgrades().await {
            tracing::debug!(%error, "a preview connection failed");
        }
    });

    Ok(sender)
}

async fn tunnel(client: OnUpgrade, vm: OnUpgrade, shutdown: CancellationToken) {
    let (client, vm) = match tokio::try_join!(client, vm) {
        Ok(upgraded) => upgraded,
        Err(error) => {
            tracing::debug!(%error, "upgrading a preview connection");
            return;
        },
    };

    let mut client = TokioIo::new(client);
    let mut vm = TokioIo::new(vm);

    tokio::select! {
        () = shutdown.cancelled() => {},
        _ = tokio::io::copy_bidirectional(&mut client, &mut vm) => {},
    }
}

fn with_query(path: &str, uri: &Uri) -> String {
    uri.query()
        .map_or_else(|| path.to_owned(), |query| format!("{path}?{query}"))
}

fn redirect(path: &str, uri: &Uri) -> Response {
    Redirect::temporary(&with_query(path, uri)).into_response()
}

fn failed(error: ApiError) -> Response {
    let mut response = error.into_response();

    if matches!(
        response.status(),
        StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE
    ) {
        response
            .headers_mut()
            .insert(REFRESH, HeaderValue::from_static("1"));
    }

    response
}

fn bad_gateway(error: hyper::Error) -> ApiError {
    ApiError::new(
        StatusCode::BAD_GATEWAY,
        format!("the VM preview did not answer: {error}"),
    )
}
