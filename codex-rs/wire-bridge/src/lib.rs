//! Responses API ↔ Chat Completions wire bridge for third-party providers.

pub mod common;
pub mod error;
pub mod history;
pub mod json_canonical;
pub mod model_helpers;
pub mod sse;
pub mod streaming;
pub mod transform;

pub use error::BridgeError;
pub use history::CodexChatHistoryStore;
pub use history::record_responses_sse_stream;
pub use streaming::create_responses_sse_stream_from_chat;
pub use streaming::create_responses_sse_stream_from_chat_with_context;
pub use transform::chat_completion_to_response;
pub use transform::chat_completion_to_response_with_context;
pub use transform::responses_to_chat_completions;
pub use transform::responses_to_chat_completions_with_reasoning;
