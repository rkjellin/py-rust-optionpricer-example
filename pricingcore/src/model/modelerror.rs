use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("Generical error")]
    ModelError,
}
