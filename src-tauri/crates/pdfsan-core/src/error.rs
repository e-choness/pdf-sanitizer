use thiserror::Error;

#[derive(Debug, Error)]
pub enum SanitizeError {
    #[error("File not found.")]
    NotFound,
    #[error("Not a PDF file (missing %PDF header).")]
    NotAPdf,
    #[error("Encrypted PDF – can't be sanitized without the password.")]
    Encrypted,
    #[error("Couldn't parse this PDF: {0}")]
    Parse(String),
    #[error("Disk error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Sanitized result failed verification; original left untouched. ({0})")]
    VerificationFailed(String),
    #[error("Stopped.")]
    Cancelled,
    #[error("Backup folder is not writable ({0}). Original left untouched.")]
    BackupFolderUnwritable(String),
}
