//! Core domain types and configuration for AnvilML.

pub mod config;
pub mod error;

pub use config::ServerConfig;
pub use error::AnvilError;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert!(true);
    }
}
