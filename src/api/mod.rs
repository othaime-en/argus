pub mod github;
pub mod gitlab;
pub mod jenkins;
pub mod traits;

pub use github::GitHubClient;
pub use gitlab::GitLabClient;
pub use jenkins::JenkinsClient;
pub use traits::{CIPlatform, LogEntry};