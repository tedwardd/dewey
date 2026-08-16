use std::fmt;

/// Application error carrying its CLI exit code.
#[derive(Debug)]
pub enum CliError {
    Usage(String),
    Module(String),
    Network(String),
    Config(String),
}

impl CliError {
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::Usage(_) => 1,
            CliError::Module(_) => 2,
            CliError::Network(_) => 3,
            CliError::Config(_) => 4,
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::Usage(m) => write!(f, "{m}"),
            CliError::Module(m) => write!(f, "{m}"),
            CliError::Network(m) => write!(f, "{m}"),
            CliError::Config(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        CliError::Network(format!("io: {e}"))
    }
}

impl From<ureq::Error> for CliError {
    fn from(e: ureq::Error) -> Self {
        CliError::Network(format!("http: {e}"))
    }
}

impl From<serde_json::Error> for CliError {
    fn from(e: serde_json::Error) -> Self {
        CliError::Module(format!("module protocol: {e}"))
    }
}

impl From<toml::de::Error> for CliError {
    fn from(e: toml::de::Error) -> Self {
        CliError::Config(format!("config: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::CliError;

    #[test]
    fn exit_codes_map_to_spec_values() {
        assert_eq!(CliError::Usage("u".into()).exit_code(), 1);
        assert_eq!(CliError::Module("m".into()).exit_code(), 2);
        assert_eq!(CliError::Network("n".into()).exit_code(), 3);
        assert_eq!(CliError::Config("c".into()).exit_code(), 4);
    }

    #[test]
    fn display_prints_message() {
        assert_eq!(CliError::Module("boom".into()).to_string(), "boom");
    }

    #[test]
    fn error_trait_implemented() {
        fn assert_error<E: std::error::Error>() {}
        assert_error::<CliError>();
    }
}
