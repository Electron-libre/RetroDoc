use crate::CompletionRequest;

mod config;
mod retry;

pub(super) fn request() -> CompletionRequest {
    CompletionRequest {
        messages: Vec::new(),
        model: None,
        json_schema: None,
    }
}
