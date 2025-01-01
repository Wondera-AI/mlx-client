#[macro_export]
macro_rules! command {
    (
        $(#[desc = $desc:literal])?
        $name:ident {
            $(
                #[arg($($attr:tt)*)]
                $field:ident: $type:ty
            ),* $(,)?
        } => $handler:ident
    ) => {
        #[derive(Debug, clap::Parser)]
        $(#[command(about = $desc)])?
        pub struct $name {
            $(
                #[arg($($attr)*)]
                pub $field: $type,
            )*
        }

        #[async_trait::async_trait]
        impl crate::Handler for $handler {
            type Command = $name;
            type Error = crate::CommandError;

            async fn handle(command: Self::Command) -> Result<(), error_stack::Report<Self::Error>> {
                Self::execute(command)
                    .await
                    .change_context(CommandError::Execution(stringify!($name).to_string()))
            }
        }
    };
}

#[macro_export]
macro_rules! command_group {
    (
        $(#[desc = $desc:literal])?
        $vis:vis enum $name:ident {
            $(
                #[desc = $subdesc:literal]
                $variant:ident($subcommand:ty)
            ),* $(,)?
        }
    ) => {
        #[derive(Debug, clap::Subcommand)]
        $(#[command(about = $desc)])?
        $vis enum $name {
            $(
                #[command(about = $subdesc)]
                $variant($subcommand),
            )*
        }

        impl $name {
            pub async fn execute(self) -> Result<(), error_stack::Report<crate::error::CommandError>> {
                match self {
                    $(
                        Self::$variant(cmd) => {
                            use crate::command::Handler;
                            paste::paste! {
                                [<$variant Handler>]::handle(cmd).await
                            }
                        }
                    ),*
                }
            }
        }
    };
}

// #[macro_export]
// macro_rules! command {
//     (
//         #[desc = $desc:expr]
//         $name:ident {
//             $(
//                 #[arg($($attr:tt)*)]
//                 $field:ident: $type:ty $(= $default:expr)?
//             ),* $(,)?
//         } => $handler:ty
//     ) => {
//         #[derive(clap::Parser, Debug)]
//         #[command(about = $desc)]
//         pub struct $name {
//             $(
//                 #[arg(
//                     long,
//                     $($attr)*
//                     $(, default_value_t = $default)?
//                 )]
//                 pub $field: $type,
//             )*
//         }

//         impl $name {
//             pub async fn execute(self) -> error_stack::Result<(), crate::CommandError> {
//                 // Check each field that's neither Optional nor has a default value
//                 $(
//                     let type_str = std::any::type_name::<$type>();
//                     // Check if type is NOT an Option AND this field has no default value
//                     if !type_str.starts_with("core::option::Option") && stringify!($($default)?).is_empty() {
//                         return Err(error_stack::Report::new(crate::CommandError::Usage(
//                             format!("Required field '{}' must be specified", stringify!($field))
//                         )));
//                     }
//                 )*
//                 <$handler as crate::CommandHandler<$name>>::handle(self).await
//             }
//         }
//     };
// }

// #[macro_export]
// macro_rules! command_group {
//     (
//         #[desc = $group_desc:expr]
//         pub enum $name:ident {
//             $(
//                 #[desc = $variant_desc:expr]
//                 $variant:ident($cmd_type:ty)
//             ),* $(,)?
//         }
//     ) => {
//         #[derive(clap::Subcommand, Debug)]
//         #[command(about = $group_desc)]
//         pub enum $name {
//             $(
//                 #[command(about = $variant_desc)]
//                 $variant($cmd_type),
//             )*
//         }

//         impl $name {
//             pub async fn execute(self) -> error_stack::Result<(), crate::CommandError> {
//                 match self {
//                     $(
//                         $name::$variant(cmd) => cmd.execute().await,
//                     )*
//                 }
//             }
//         }
//     };
// }
