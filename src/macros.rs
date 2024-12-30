use async_trait::async_trait;
use error_stack::Result;

#[macro_export]
macro_rules! define_error {
    (
        pub enum $name:ident {
            $($variant:ident($message:expr)),* $(,)?
        }
    ) => {
        #[derive(Debug, thiserror::Error)]
        pub enum $name {
            $(
                #[error($message)]
                $variant(String),
            )*

            #[error(transparent)]
            Reqwest(#[from] reqwest::Error),
            #[error(transparent)]
            Io(#[from] std::io::Error),
            #[error(transparent)]
            SerdeJson(#[from] serde_json::Error),
            #[error(transparent)]
            TomlDe(#[from] toml::de::Error),
        }

        impl $name {
            pub fn into_report<T>(error: impl Into<Self>) -> error_stack::Result<T, Self> {
                Err(error_stack::Report::new(error.into()))
            }
        }
    };

    (
        pub enum $name:ident : $parent:ty {
            $($variant:ident($message:expr)),* $(,)?
        }
    ) => {
        #[derive(Debug, thiserror::Error)]
        pub enum $name {
            $(
                #[error($message)]
                $variant(String),
            )*

            #[error(transparent)]
            Parent(#[from] $parent),

            #[error(transparent)]
            Reqwest(#[from] reqwest::Error),
            #[error(transparent)]
            Io(#[from] std::io::Error),
            #[error(transparent)]
            SerdeJson(#[from] serde_json::Error),
            #[error(transparent)]
            TomlDe(#[from] toml::de::Error),
        }

        impl From<$name> for $parent {
            fn from(error: $name) -> Self {
                match error {
                    $name::Parent(e) => e,
                    _ => <$parent>::Operation(error.to_string()),
                }
            }
        }
    };
}

#[async_trait]
pub trait CommandHandler<C> {
    type Error;
    type Output;

    async fn handle(command: C) -> Result<Self::Output, Self::Error>;
}

#[macro_export]
macro_rules! command {
    (
        #[desc = $cmd_desc:expr]
        $name:ident<$error:ty, $output:ty> {
            $(
                #[desc = $field_desc:expr]
                $(#[$field_meta:meta])*
                $field:ident: $type:ty $(= $default:expr)?
            ),* $(,)?
        } => $handler:ty
    ) => {
        #[derive(clap::Parser)]
        #[command(about = $cmd_desc)]
        pub struct $name {
            $(
                #[arg(
                    long,
                    help = $field_desc,
                    $(default_value_t = $default,)?
                )]
                $(#[$field_meta])*
                pub $field: $type,
            )*
        }

        impl $name {
            pub async fn execute(self) -> error_stack::Result<$output, $error> {
                <$handler as $crate::macros::CommandHandler<$name>>::handle(self).await
            }
        }

        #[async_trait::async_trait]
        impl $crate::macros::CommandHandler<$name> for $handler {
            type Error = $error;
            type Output = $output;

            async fn handle(command: $name) -> error_stack::Result<Self::Output, Self::Error> {
                Self::execute(command).await
            }
        }
    };
}

#[macro_export]
macro_rules! command_group {
    (
        #[desc = $group_desc:expr]
        pub enum $name:ident {
            $(
                #[desc = $variant_desc:expr]
                $variant:ident($type:ty)
            ),* $(,)?
        }
    ) => {
        #[derive(clap::Subcommand)]
        #[command(about = $group_desc)]
        pub enum $name {
            $(
                #[command(about = $variant_desc)]
                $variant($type),
            )*
        }

        impl $name {
            pub async fn execute(self) -> error_stack::Result<(), super::CommandError> {
                match self {
                    $(
                        $name::$variant(cmd) => cmd.execute()
                            .await
                            .change_context(super::CommandError::Operation(
                                format!("{} command failed", stringify!($variant))
                            )),
                    )*
                }
            }
        }
    };
}

// #[macro_export]
// macro_rules! command_group {
//     (
//         #[desc = $group_desc:expr]
//         pub enum $name:ident {
//             $(
//                 #[desc = $variant_desc:expr]
//                 $variant:ident($type:ty)
//             ),* $(,)?
//         }
//     ) => {
//         #[derive(clap::Subcommand)]
//         #[command(about = $group_desc)]
//         pub enum $name {
//             $(
//                 #[command(about = $variant_desc)]
//                 $variant($type),
//             )*
//         }

//         impl $name {
//             pub async fn execute(self) -> error_stack::Result<(), super::CommandError> {
//                 match self {
//                     $(
//                         // All specific errors like DeployError will map to CommandError
//                         // through the From implementations our define_error! macro created
//                         $name::$variant(cmd) => cmd.execute().await.map_err(Into::into),
//                     )*
//                 }
//             }
//         }
//     };
// }

// #[async_trait]
// pub trait CommandHandler<C> {
//     type Error;
//     type Output;

//     async fn handle(command: C) -> error_stack::Result<Self::Output, Self::Error>;
// }

// #[macro_export]
// macro_rules! command {
//     (
//         #[desc = $cmd_desc:expr]
//         $name:ident<$error:ty, $output:ty> {
//             $(
//                 #[desc = $field_desc:expr]
//                 $(#[$field_meta:meta])*
//                 $field:ident: $type:ty $(= $default:expr)?
//             ),* $(,)?
//         } => $handler:ty
//     ) => {
//         #[derive(clap::Parser)]
//         #[command(about = $cmd_desc)]
//         pub struct $name {
//             $(
//                 #[arg(
//                     long,
//                     help = $field_desc,
//                     $(default_value_t = $default,)?
//                 )]
//                 $(#[$field_meta])*
//                 pub $field: $type,
//             )*
//         }

//         impl $name {
//             pub async fn execute(self) -> error_stack::Result<$output, $error> {
//                 <$handler as $crate::macros::CommandHandler<$name>>::handle(self).await
//             }
//         }

//         #[async_trait::async_trait]
//         impl $crate::macros::CommandHandler<$name> for $handler {
//             type Error = $error;
//             type Output = $output;
//         }
//     };
// }
