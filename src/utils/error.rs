use thiserror::Error;

/// Main error type for ARGUS application
#[derive(Error, Debug)]
pub enum ArgusError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("API error: {0}")]
    Api(#[from] ApiError),

    #[error("UI error: {0}")]
    Ui(#[from] UiError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Configuration-related errors
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to load configuration file: {0}")]
    LoadFailed(String),

    #[error("Failed to parse configuration: {0}")]
    ParseFailed(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Missing environment variable: {0}")]
    MissingEnvVar(String),

    #[error("Config file not found at path: {0}")]
    FileNotFound(String),
}

/// API communication errors
#[derive(Error, Debug)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(String),

    #[error("Failed to parse response: {0}")]
    ParseFailed(String),

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("API endpoint not found: {0}")]
    NotFound(String),

    #[error("API returned error: {status} - {message}")]
    ApiError { status: u16, message: String },

    #[error("Network error: {0}")]
    Network(String),

    #[error("Timeout: {0}")]
    Timeout(String),
}

/// Terminal/UI errors
#[derive(Error, Debug)]
pub enum UiError {
    #[error("Failed to initialize terminal: {0}")]
    InitFailed(String),

    #[error("Failed to render UI: {0}")]
    RenderFailed(String),

    #[error("Terminal error: {0}")]
    Terminal(String),

    #[error("Failed to handle input: {0}")]
    InputFailed(String),
}

/// Result type alias for ARGUS operations
pub type Result<T> = std::result::Result<T, ArgusError>;

/// Helper trait to add context to errors
pub trait ResultExt<T> {
    fn context(self, ctx: &str) -> Result<T>;
}

impl<T, E> ResultExt<T> for std::result::Result<T, E>
where
    E: Into<ArgusError>,
{
    fn context(self, ctx: &str) -> Result<T> {
        self.map_err(|e| {
            let base_error = e.into();
            match base_error {
                ArgusError::Config(e) => {
                    ArgusError::Config(ConfigError::InvalidConfig(format!("{}: {}", ctx, e)))
                }
                ArgusError::Api(e) => {
                    ArgusError::Api(ApiError::RequestFailed(format!("{}: {}", ctx, e)))
                }
                ArgusError::Ui(e) => {
                    ArgusError::Ui(UiError::RenderFailed(format!("{}: {}", ctx, e)))
                }
                _ => ArgusError::Unknown(format!("{}: {}", ctx, base_error)),
            }
        })
    }
}
