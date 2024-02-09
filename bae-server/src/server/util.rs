use axum::response::{IntoResponse, Response};
use axum_extra::headers::ContentType;
use axum_extra::TypedHeader;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct Xml<T>(pub T);

impl<T: IntoResponse> IntoResponse for Xml<T> {
    fn into_response(self) -> Response {
        (TypedHeader(ContentType::xml()), self.0).into_response()
    }
}
