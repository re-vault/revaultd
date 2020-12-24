pub mod actions;
// We don't use all the interfaces just yet
#[allow(dead_code)]
pub mod interface;
mod schema;

#[derive(PartialEq, Eq, Debug)]
pub struct DatabaseError(pub String);

impl std::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Database error: {}", self.0)
    }
}

impl std::error::Error for DatabaseError {}

impl From<revault_tx::Error> for DatabaseError {
    fn from(e: revault_tx::Error) -> Self {
        Self(format!("Transaction error: {}", e))
    }
}

pub const DB_VERSION: u32 = 0;
