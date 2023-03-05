//! This module holds the middleware definitions

pub(crate) use authentication_required::AuthenticationRequired;
pub(crate) use handle_not_found::handle_not_found;
pub(crate) use json_extractor_error::json_extractor_error;

mod authentication_required;
mod handle_not_found;
mod json_extractor_error;
