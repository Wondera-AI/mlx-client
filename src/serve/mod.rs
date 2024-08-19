pub mod create;
pub mod delete;
pub mod jobs;
pub mod list;
pub mod log;
pub mod scale;

pub use create::*;
pub use delete::*;
pub use jobs::*;
pub use list::*;
pub use log::*;
pub use scale::*;

static SERVER_URL: &str = "http://localhost:3000";
