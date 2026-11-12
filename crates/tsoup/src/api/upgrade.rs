use axum::extract::Request;
use axum::http::header::{CONNECTION, UPGRADE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse as _, Response};
use hyper::upgrade::OnUpgrade;

use super::ApiError;

pub fn accept(
    request: &mut Request,
    protocol: &'static str,
) -> Result<(OnUpgrade, Response), ApiError> {
    if !request.headers().get(UPGRADE).is_some_and(|requested| {
        requested
            .as_bytes()
            .eq_ignore_ascii_case(protocol.as_bytes())
    }) {
        return Err(ApiError::new(
            StatusCode::UPGRADE_REQUIRED,
            format!("this endpoint speaks the {protocol} protocol"),
        ));
    }

    let response = (
        StatusCode::SWITCHING_PROTOCOLS,
        [
            (CONNECTION, HeaderValue::from_static("upgrade")),
            (UPGRADE, HeaderValue::from_static(protocol)),
        ],
    )
        .into_response();

    Ok((hyper::upgrade::on(request), response))
}
