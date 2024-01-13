use askama::Template;
use axum::http::StatusCode;

#[derive(Template)]
#[template(path = "error.html")]
pub struct ErrorTemplate {
    pub(crate) status: StatusCode,
}

#[derive(Template)]
#[template(path = "home.html")]
pub struct HomeTemplate {}
