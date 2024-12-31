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
                #[error("{}: {}", $message, _0)]
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
                #[error("{}: {}", $message, _0)]
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
        #[desc = $desc:expr]
        $name:ident<$error:ty, $output:ty> {
            $(
                #[arg($($attr:tt)*)]
                $field:ident: $type:ty $(= $default:expr)?
            ),* $(,)?
        } => $handler:ty
    ) => {

        #[derive(clap::Parser, Debug)]
        #[command(about = $desc)]
        pub struct $name {
            $(
                #[arg(
                    long,
                    $($attr)*
                    $(, default_value_t = $default)?
                )]
                pub $field: $type,
            )*
        }

        impl $name {
            pub async fn execute(self) -> error_stack::Result<$output, $error> {
                // default value checks
                $(
                    let type_str = std::any::type_name::<$type>();
                    if !type_str.starts_with("core::option::Option") {
                        if stringify!($($default)?).is_empty() {
                            let detail = format!(
                                "Field '{}' of type '{}' requires a default value",
                                stringify!($field),
                                stringify!($type)
                            );

                            return Err(error_stack::Report::new(<$error>::Parent(
                                crate::CommandError::Config(detail)
                            )));
                        }
                    }
                )*

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
// #[macro_export]
// macro_rules! command {
//     (
//         #[desc = $cmd_desc:expr]
//         $name:ident<$error:ty, $output:ty> {
//             $(
//                 $(#[$field_meta:meta])*
//                 $field:ident: $type:ty $(= $default:expr)?
//             ),* $(,)?
//         } => $handler:ty
//     ) => {
//         #[derive(clap::Parser, Debug)]
//         #[command(about = $cmd_desc)]
//         pub struct $name {
//             $(
//                 #[arg(
//                     long,
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

//             async fn handle(command: $name) -> error_stack::Result<Self::Output, Self::Error> {
//                 <$handler>::execute(command).await
//             }
//         }
//     };
// }

// #[macro_export]
// macro_rules! conditionally_add_value_parser {
//     (std::collections::HashMap<String, String>) => {
//         parse_key_value_pairs
//     };

//     ($type:ty) => {};
// }

// pub fn parse_hashmap(s: &str) -> Result<HashMap<String, String>, crate::CommandError> {
//     s.split(',').map(|item| parse_key_val(item)).collect()
// }

// pub fn parse_key_val(s: &str) -> Result<(String, String), crate::CommandError> {
//     let pos = s.find('=').ok_or_else(|| {
//         crate::CommandError::UsageError(format!("invalid KEY=value: no `=` found in `{}`", s))
//     })?;
//     let key = s[..pos].to_string();
//     let value = s[pos + 1..].to_string();
//     Ok((key, value))
// }

// // #[macro_export]
// // macro_rules! command {
// //     (
// //         #[desc = $cmd_desc:expr]
// //         $name:ident<$error:ty, $output:ty> {
// //             $(
// //                 #[desc = $field_desc:expr]
// //                 $(#[$field_meta:meta])*
// //                 $field:ident: $type:ty $(= $default:expr)?
// //             ),* $(,)?
// //         } => $handler:ty
// //     ) => {
// //         #[derive(clap::Parser, Debug)]
// //         #[command(about = $cmd_desc)]
// //         pub struct $name {
// //             $(
// //                 #[arg(
// //                     long,
// //                     help = $field_desc,
// //                     $(default_value_t = $default,)?
// //                     $(value_parser = $crate::macros::parse_value_parser::<$type>)*
// //                 )]
// //                 $(#[$field_meta])*
// //                 pub $field: $type,
// //             )*
// //         }

// //         impl $name {
// //             pub async fn execute(self) -> error_stack::Result<$output, $error> {
// //                 <$handler as $crate::macros::CommandHandler<$name>>::handle(self).await
// //             }
// //         }

// //         #[async_trait::async_trait]
// //         impl $crate::macros::CommandHandler<$name> for $handler {
// //             type Error = $error;
// //             type Output = $output;

// //             async fn handle(command: $name) -> error_stack::Result<Self::Output, Self::Error> {
// //                 <$handler>::execute(command).await
// //             }
// //         }
// //     };
// // }

// pub fn internal_value_parser<T>() -> Option<clap::builder::ValueParser>
// where
//     T: 'static + FromStr + Send + Sync + std::clone::Clone,
//     T: FromStr,
//     T::Err: std::error::Error + Send + Sync + 'static + std::clone::Clone,
// {
//     let type_name = std::any::type_name::<T>();
//     if type_name.starts_with("std::collections::hash::map::HashMap") {
//         Some(clap::builder::ValueParser::new(parse_hashmap))
//     } else if type_name.starts_with("core::option::Option<std::collections::hash::map::HashMap") {
//         Some(clap::builder::ValueParser::new(parse_option_hashmap))
//     } else {
//         None
//     }
// }

// pub fn dynamic_from_str<T>(s: &str) -> Result<T, crate::CommandError>
// where
//     T: FromStr + Send + Sync,
//     T::Err: std::error::Error + Send + Sync + 'static,
// {
//     Ok(T::from_str(s)
//         .map_err(|e| crate::CommandError::UsageError(format!("Failed to parse: {e}")))?)
// }

// fn is_hashmap<T>() -> bool
// where
//     T: 'static,
// {
//     let type_name = std::any::type_name::<T>();
//     type_name.starts_with("std::collections::hash::map::HashMap<")
// }

// fn is_option_hashmap<T>() -> bool
// where
//     T: 'static,
// {
//     let type_name = std::any::type_name::<T>();
//     type_name.starts_with("core::option::Option<std::collections::hash::map::HashMap<")
// }

// pub fn internal_value_parser<T>() -> clap::builder::ValueParser
// where
//     T: 'static,
// {
//     // Handle `Option<T>` by delegating to the parser for `T`
//     if is_option::<T>() {
//         // Extract the inner type of `Option<T>`
//         let inner_parser = match inner_type::<T>().as_str() {
//             "bool" => clap::builder::ValueParser::bool(),
//             "alloc::string::String" => clap::builder::ValueParser::string(),
//             "u32" => clap::builder::ValueParser::new(u32::from_str),
//             "f32" => clap::builder::ValueParser::new(f32::from_str),
//             "i32" => clap::builder::ValueParser::new(i32::from_str),
//             "std::collections::HashMap<alloc::string::String, alloc::string::String>" => {
//                 clap::builder::ValueParser::new(parse_hashmap)
//             }
//             _ => panic!("Unsupported Option inner type: {}", inner_type::<T>()),
//         };
//         return inner_parser;
//     }

//     // Handle non-`Option` types directly
//     if std::any::TypeId::of::<T>() == std::any::TypeId::of::<bool>() {
//         clap::builder::ValueParser::bool()
//     } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<String>() {
//         clap::builder::ValueParser::string()
//     } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<u32>() {
//         clap::builder::ValueParser::new(u32::from_str)
//     } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<f32>() {
//         clap::builder::ValueParser::new(f32::from_str)
//     } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<i32>() {
//         clap::builder::ValueParser::new(i32::from_str)
//     } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<HashMap<String, String>>() {
//         clap::builder::ValueParser::new(parse_hashmap)
//     } else {
//         panic!(
//             "Unsupported argument type for clap value parser: {:?}",
//             std::any::type_name::<T>()
//         );
//     }
// }

// /// Checks if a type is `Option<T>`
// fn is_option<T>() -> bool
// where
//     T: 'static,
// {
//     std::any::type_name::<T>().starts_with("core::option::Option<")
// }

// /// Extracts the inner type name of `Option<T>` as a `String`
// fn inner_type<T>() -> String
// where
//     T: 'static,
// {
//     let type_name = std::any::type_name::<T>();
//     if is_option::<T>() {
//         type_name
//             .trim_start_matches("core::option::Option<")
//             .trim_end_matches('>')
//             .to_string()
//     } else {
//         type_name.to_string()
//     }
// }

// #[derive(Debug, thiserror::Error)]
// #[error("Parsing error occurred: {source}")]
// pub struct ParseWrapperError {
//     #[from]
//     source: crate::CommandError,
// }

// pub fn parse_key_val<K, V>(s: &str) -> Result<(K, V), crate::CommandError>
// where
//     K: FromStr,
//     K::Err: std::error::Error + Send + Sync + 'static,
//     V: FromStr,
//     V::Err: std::error::Error + Send + Sync + 'static,
// {
//     let pos = s.find('=').ok_or_else(|| {
//         crate::CommandError::UsageError(format!("invalid KEY=value: no `=` found in `{}`", s))
//     })?;

//     let key = s[..pos]
//         .parse::<K>()
//         .map_err(|e| crate::CommandError::UsageError(format!("Failed to parse key: {}", e)))?;

//     let value = s[pos + 1..]
//         .parse::<V>()
//         .map_err(|e| crate::CommandError::UsageError(format!("Failed to parse value: {}", e)))?;

//     Ok((key, value))
// }

// pub fn parse_hashmap(s: &str) -> Result<HashMap<String, String>, crate::CommandError> {
//     let mut map = HashMap::new();
//     for item in s.split(',') {
//         let (key, value) = parse_key_val(item)?;
//         map.insert(key, value);
//     }
//     Ok(map)
// }

// pub fn internal_value_parser<T>() -> ValueParser
// where
//     T: 'static,
// {
//     // Special case: Handle `HashMap` or `Option<HashMap>`
//     if is_hashmap::<T>() || is_option_hashmap::<T>() {
//         return ValueParser::new(parse_option_hashmap);
//     }

//     // For all other types, rely on Clap's `from_str`
//     ValueParser::from_str::<T>()
// }

// /// Determines if a type is a `HashMap<K, V>`
// fn is_hashmap<T>() -> bool
// where
//     T: 'static,
// {
//     let type_name = std::any::type_name::<T>();
//     type_name.starts_with("std::collections::hash::map::HashMap<")
// }

// /// Determines if a type is `Option<HashMap<K, V>>`
// fn is_option_hashmap<T>() -> bool
// where
//     T: 'static,
// {
//     let type_name = std::any::type_name::<T>();
//     type_name.starts_with("core::option::Option<std::collections::hash::map::HashMap<")
// }

// /// Custom parser for `Option<HashMap<String, String>>`
// pub fn parse_option_hashmap(
//     s: &str,
// ) -> Result<Option<HashMap<String, String>>, crate::CommandError> {
//     if s.is_empty() {
//         return Ok(None);
//     }

//     parse_hashmap(s).map(Some)
// }

// /// Custom parser for `HashMap<String, String>`
// pub fn parse_hashmap(s: &str) -> Result<HashMap<String, String>, crate::CommandError> {
//     s.split(',').map(|item| parse_key_val(item)).collect()
// }

// pub fn internal_value_parser<T>() -> clap::builder::ValueParser
// where
//     T: 'static,
// {
//     // Handle `Option<HashMap<String, String>>` explicitly
//     if std::any::type_name::<T>() == "core::option::Option<std::collections::hash::map::HashMap<alloc::string::String, alloc::string::String>>" {
//         return clap::builder::ValueParser::new(parse_hashmap);
//     }

//     // Handle `Option<T>` generically
//     if is_option::<T>() {
//         // Extract the inner type of `Option<T>`
//         return match inner_type::<T>().as_str() {
//             "bool" => clap::builder::ValueParser::bool(),
//             "alloc::string::String" => clap::builder::ValueParser::string(),
//             "u32" => clap::builder::ValueParser::new(u32::from_str),
//             "f32" => clap::builder::ValueParser::new(f32::from_str),
//             "i32" => clap::builder::ValueParser::new(i32::from_str),
//             _ => panic!("Unsupported Option inner type: {}", inner_type::<T>()),
//         };
//     }

//     // Handle non-`Option` types
//     direct_value_parser::<T>()
// }

// /// Provides a `ValueParser` for non-`Option` types
// fn direct_value_parser<T>() -> clap::builder::ValueParser
// where
//     T: 'static,
// {
//     if std::any::TypeId::of::<T>() == std::any::TypeId::of::<bool>() {
//         clap::builder::ValueParser::bool()
//     } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<String>() {
//         clap::builder::ValueParser::string()
//     } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<u32>() {
//         clap::builder::ValueParser::new(u32::from_str)
//     } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<f32>() {
//         clap::builder::ValueParser::new(f32::from_str)
//     } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<i32>() {
//         clap::builder::ValueParser::new(i32::from_str)
//     } else if std::any::TypeId::of::<T>() == std::any::TypeId::of::<HashMap<String, String>>() {
//         clap::builder::ValueParser::new(parse_hashmap)
//     } else {
//         panic!(
//             "Unsupported argument type for clap value parser: {:?}",
//             std::any::type_name::<T>()
//         );
//     }
// }

// /// Detects if a type is `Option<T>` and returns `true` if so
// fn is_option<T>() -> bool
// where
//     T: 'static,
// {
//     std::any::type_name::<T>().starts_with("core::option::Option<")
// }

// /// Extracts the inner type of an `Option<T>` as a string
// fn inner_type<T>() -> String
// where
//     T: 'static,
// {
//     let type_name = std::any::type_name::<T>();
//     if is_option::<T>() {
//         type_name
//             .trim_start_matches("core::option::Option<")
//             .trim_end_matches('>')
//             .to_string()
//     } else {
//         type_name.to_string()
//     }
// }

// /// Parses a `HashMap` from a comma-separated `key=value` list
// pub fn parse_hashmap(s: &str) -> Result<HashMap<String, String>, crate::CommandError> {
//     s.split(',').map(|item| parse_key_val(item)).collect()
// }

// /// Parses a single `key=value` pair
// pub fn parse_key_val(s: &str) -> Result<(String, String), crate::CommandError> {
//     let pos = s.find('=').ok_or_else(|| {
//         crate::CommandError::UsageError(format!("invalid KEY=value: no `=` found in `{}`", s))
//     })?;
//     let key = s[..pos].to_string();
//     let value = s[pos + 1..].to_string();
//     Ok((key, value))
// }

// pub fn parse_key_val<T, U>(
//     s: &str,
// ) -> Result<(T, U), Box<dyn std::error::Error + Send + Sync + 'static>>
// where
//     T: std::str::FromStr,
//     T::Err: std::error::Error + Send + Sync + 'static,
//     U: std::str::FromStr,
//     U::Err: std::error::Error + Send + Sync + 'static,
// {
//     let pos = s
//         .find('=')
//         .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;
//     Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
// }

// pub trait IsOptional {
//     const VALUE: bool;
// }

// impl<T> IsOptional for Option<T> {
//     const VALUE: bool = true;
// }

// impl<T> IsOptional for std::collections::HashMap<T, T> {
//     const VALUE: bool = false;
// }

// impl<T> IsOptional for Vec<T> {
//     const VALUE: bool = false;
// }

// macro_rules! impl_is_optional {
//     ($($t:ty),*) => {
//         $(
//             impl IsOptional for $t {
//                 const VALUE: bool = false;
//             }
//         )*
//     }
// }

// impl_is_optional!(bool, i8, i16, i32, i64, u8, u16, u32, u64, f32, f64, String, str);

// pub trait IsHashMap {
//     const IS_HASHMAP: bool = false;
// }

// impl<K, V> IsHashMap for HashMap<K, V> {
//     const IS_HASHMAP: bool = true;
// }

// impl<T> IsHashMap for Option<T>
// where
//     T: IsHashMap,
// {
//     const IS_HASHMAP: bool = T::IS_HASHMAP;
// }

// // // Parse key-value strings for HashMap fields
// // pub fn parse_key_val<K: FromStr, V: FromStr>(
// //     s: &str,
// // ) -> std::result::Result<(K, V), Box<dyn std::error::Error + Send + Sync>>
// // where
// //     K::Err: std::error::Error + Send + Sync + 'static,
// //     V::Err: std::error::Error + Send + Sync + 'static,
// // {
// //     let pos = s.find('=').ok_or_else(|| {
// //         Box::new(std::io::Error::new(
// //             std::io::ErrorKind::InvalidInput,
// //             format!("invalid KEY=value: no `=` found in `{s}`"),
// //         ))
// //     })?;
// //     Ok((
// //         s[..pos].parse().map_err(|e| Box::new(e))?,
// //         s[pos + 1..].parse().map_err(|e| Box::new(e))?,
// //     ))
// // }
// // First, let's create a wrapper for our errors that implements Context
// #[derive(Debug)]
// struct ParseError(Box<dyn std::error::Error + Send + Sync>);

// impl std::fmt::Display for ParseError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "{}", self.0)
//     }
// }

// impl std::error::Error for ParseError {
//     fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
//         self.0.source()
//     }
// }

// impl error_stack::Context for ParseError {}

// // Now we can use this wrapper in our parse function
// pub fn parse_key_val<K: FromStr, V: FromStr>(s: &str) -> error_stack::Result<(K, V), ParseError>
// where
//     K::Err: std::error::Error + Send + Sync + 'static,
//     V::Err: std::error::Error + Send + Sync + 'static,
// {
//     let pos = s.find('=').ok_or_else(|| {
//         let err = std::io::Error::new(
//             std::io::ErrorKind::InvalidInput,
//             format!("invalid KEY=value: no `=` found in `{s}`"),
//         );
//         error_stack::Report::new(ParseError(Box::new(err)))
//     })?;

//     let key = s[..pos]
//         .parse::<K>()
//         .map_err(|e| error_stack::Report::new(ParseError(Box::new(e))))?;

//     let value = s[pos + 1..]
//         .parse::<V>()
//         .map_err(|e| error_stack::Report::new(ParseError(Box::new(e))))?;

//     Ok((key, value))
// }

// // Update the trait to use our ParseError
// pub trait KeyValueParser {
//     fn parse_key_value(input: &str) -> error_stack::Result<Self, ParseError>
//     where
//         Self: Sized;
// }

// impl KeyValueParser for HashMap<String, String> {
//     fn parse_key_value(input: &str) -> error_stack::Result<Self, ParseError> {
//         let mut map = HashMap::new();
//         for pair in input.split(',') {
//             let (key, value) = parse_key_val(pair)?;
//             map.insert(key, value);
//         }
//         Ok(map)
//     }
// }

// #[macro_export]
// macro_rules! command {
//     (
//         #[desc = $cmd_desc:expr]
//         $name:ident<$error:ty, $output:ty> {
//             // Pattern matching for HashMaps specifically
//             $(
//                 #[desc = $field_desc_hm:expr]
//                 $field_hm:ident: Option<HashMap<$k:ty, $v:ty>>
//             ),* $(,)?

//             // Pattern matching for all other fields
//             $(,
//                 #[desc = $field_desc:expr]
//                 $field:ident: $type:ty $(= $default:expr)?
//             )* $(,)?
//         } => $handler:ty
//     ) => {
//         #[derive(clap::Parser)]
//         #[command(about = $cmd_desc)]
//         pub struct $name {
//             // Generate HashMap fields with custom parser
//             $(
//                 #[arg(
//                     long,
//                     help = $field_desc_hm,
//                     value_delimiter = ',',
//                     value_parser = clap::builder::ValueParser::new(crate::macros::parse_key_val::<$k, $v>)
//                 )]
//                 pub $field_hm: Option<HashMap<$k, $v>>,
//             )*

//             // Generate regular fields
//             $(
//                 #[arg(
//                     long,
//                     help = $field_desc
//                     $(, default_value_t = $default)?
//                 )]
//                 pub $field: $type,
//             )*
//         }

//         impl $name {
//             pub async fn execute(self) -> error_stack::Result<$output, $error> {
//                 <$handler as crate::macros::CommandHandler<$name>>::handle(self).await
//             }
//         }

//         #[async_trait::async_trait]
//         impl crate::macros::CommandHandler<$name> for $handler {
//             type Error = $error;
//             type Output = $output;

//             async fn handle(command: $name) -> error_stack::Result<Self::Output, Self::Error> {
//                 Self::execute(command).await
//             }
//         }
//     };
// }

// #[macro_export]
// macro_rules! command {
//     (
//         #[desc = $desc:expr]
//         $name:ident<$error:ty, $output:ty> {
//             $(
//                 #[arg($($attr:tt)*)]
//                 $field:ident: $type:ty $(= $default:expr)?
//             ),* $(,)?
//         } => $handler:ty
//     ) => {
//         #[derive(clap::Parser)]
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
//             pub async fn execute(self) -> error_stack::Result<$output, $error> {
//                 <$handler as $crate::macros::CommandHandler<$name>>::handle(self).await
//             }
//         }

//         #[async_trait::async_trait]
//         impl $crate::macros::CommandHandler<$name> for $handler {
//             type Error = $error;
//             type Output = $output;

//             async fn handle(command: $name) -> error_stack::Result<Self::Output, Self::Error> {
//                 Self::execute(command).await
//             }
//         }
//     };
// }
