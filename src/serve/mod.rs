pub mod create;
pub mod delete;
pub mod list;

pub use create::*;
pub use delete::*;
pub use list::*;

static SERVER_URL: &str = "http://localhost:3000";
