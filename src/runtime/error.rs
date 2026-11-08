#[derive(Debug)]
pub enum ForgeError {
    Scan(String),
    Parse { message: String, line: usize },
    Runtime { message: String, line: usize },
}

impl std::fmt::Display for ForgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ForgeError::Scan(m) => write!(f, "Scan error: {}", m),
            ForgeError::Parse { message, line } => write!(f, "Parse error at line {}: {}", line, message),
            ForgeError::Runtime { message, line } => write!(f, "Runtime error at line {}: {}", line, message),
        }
    }
}

impl std::error::Error for ForgeError {}
