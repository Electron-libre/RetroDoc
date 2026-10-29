use crate::CompletionRequest;

mod completion;
mod config;
mod fallbacks;
mod retry;
mod timeouts;

pub(super) fn request() -> CompletionRequest {
    CompletionRequest {
        messages: Vec::new(),
        model: None,
        json_schema: None,
    }
}
