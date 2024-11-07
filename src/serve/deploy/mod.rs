mod deploy;
mod docker;
mod service_config;
mod service_params;

use docker::build_tag_and_push_image;

pub use deploy::deploy_service;
pub use service_config::{ResourceRequest, ServiceConfig};
pub use service_params::ServiceSchema;

#[cfg(test)]
mod tests {
    use super::*;
    use service_params::ServiceSchema;

    #[test]
    fn test_build_service_params_from_json() {
        let json_data = r#"
        {
            "input": {
                "path": [{"name": "required_foo", "dtype": "string", "required": "True"}],
                "query": [{"name": "bar", "dtype": "string", "required": "False"}],
                "body": [
                    {"name": "mtype", "dtype": "string", "required": "True"},
                    {"name": "optional_smoothing", "dtype": "integer", "required": "False"}
                ]
            },

            "output": [
                {"name": "foo", "dtype": "string", "required": "True"},
                {"name": "bar", "dtype": "string", "required": "True"}
            ]
        }
        "#;

        let result = ServiceSchema::from_json(json_data).expect("Failed to build service params");

        assert_eq!(result.input.path.as_ref().unwrap()[0].name, "required_foo");
        assert_eq!(result.input.path.as_ref().unwrap()[0].dtype, "string");
        assert!(result.input.path.as_ref().unwrap()[0].required);

        assert_eq!(result.input.query.as_ref().unwrap()[0].name, "bar");
        assert_eq!(result.input.query.as_ref().unwrap()[0].dtype, "string");
        assert!(!result.input.query.as_ref().unwrap()[0].required);

        assert_eq!(result.input.body.as_ref().unwrap()[0].name, "mtype");
        assert_eq!(result.input.body.as_ref().unwrap()[0].dtype, "string");
        assert!(result.input.body.as_ref().unwrap()[0].required);

        assert_eq!(
            result.input.body.as_ref().unwrap()[1].name,
            "optional_smoothing"
        );
        assert_eq!(result.input.body.as_ref().unwrap()[1].dtype, "integer");
        assert!(!result.input.body.as_ref().unwrap()[1].required);

        assert_eq!(result.output["foo"].name, "foo");
        assert_eq!(result.output["foo"].dtype, "string");
        assert!(result.output["foo"].required);

        assert_eq!(result.output["bar"].name, "bar");
        assert_eq!(result.output["bar"].dtype, "string");
        assert!(result.output["bar"].required);
    }
}
