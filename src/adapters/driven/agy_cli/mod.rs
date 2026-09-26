//! Antigravity CLI adapter module partitioned according to Single Responsibility Principle (SRP):
//! - `adapter`: Process runner and AgentEnginePort implementation.
//! - `account_pool`: Multi-account pool lifecycle, cooldown calculation, and quota error detection.
//! - `models`: AI model catalog and alias normalization.
//! - `oauth`: OAuth browser PKCE session directories and authorization code sanitization.
//! - `sanitizer`: Agent response post-processing and intermediate log stripping.

pub mod account_pool;
pub mod adapter;
pub mod models;
pub mod oauth;
pub mod sanitizer;

#[allow(unused_imports)]
pub use account_pool::{
    extract_email_from_token, extract_quota_cooldown_duration, is_auth_error, is_quota_error,
    is_transient_error, mask_email, AccountToken, StoredAccountToken,
};
pub use adapter::AntigravityCliAdapter;
#[allow(unused_imports)]
pub use models::{get_available_models, resolve_model_name};
#[allow(unused_imports)]
pub use oauth::{get_oauth_session_dir, sanitize_oauth_code};
#[allow(unused_imports)]
pub use sanitizer::sanitize_agent_response;
