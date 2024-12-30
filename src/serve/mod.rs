use crate::command_group;
use error_stack::ResultExt;

command_group! {
    #[desc = "Manage and control MLX services"]
    pub enum ServeCommand {
        #[desc = "Deploy a new service or update existing one"]
        Deploy(DeployCommand),

        // #[desc = "List all available services with their status"]
        // List(ListCommand),

        // #[desc = "Remove a service or specific service version"]
        // Remove(RemoveCommand),

        // #[desc = "Scale service replica count"]
        // Scale(ScaleCommand),

        // #[desc = "View service logs and execution details"]
        // Logs(LogsCommand),

        // #[desc = "List active service jobs"]
        // Jobs(JobsCommand),
    }
}

pub mod deploy;
// pub mod jobs;
// pub mod list;
// pub mod logs;
// pub mod remove;
// pub mod scale;

pub use deploy::DeployCommand;
// pub use jobs::JobsCommand;
// pub use list::ListCommand;
// pub use logs::LogsCommand;
// pub use remove::RemoveCommand;
// pub use scale::ScaleCommand;
