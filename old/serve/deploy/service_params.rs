use crate::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use utils::errors::prelude::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceInputParams {
    pub path: Option<Vec<Param>>,

    pub query: Option<Vec<Param>>,

    pub body: Option<Vec<Param>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Param {
    pub name: String,

    pub dtype: String,

    pub required: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceSchema {
    pub input: ServiceInputParams,

    pub output: HashMap<String, Param>,
}

impl ServiceSchema {
    pub fn default() -> Self {
        ServiceSchema {
            input: ServiceInputParams {
                path: None,
                query: None,
                body: None,
            },
            output: HashMap::new(),
        }
    }

    pub async fn from_json_file(file_name: &str) -> RResult<Self, AnyErr2> {
        let mut file = File::open(file_name)
            .await
            .expect("Failed to open schema.json");

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .await
            .expect("Failed to read schema.json");

        ServiceSchema::from_json(&contents)
    }

    pub fn from_json(contents: &str) -> RResult<Self, AnyErr2> {
        debug!("Contents: {:?}", contents);
        let json: Value =
            serde_json::from_str(&contents).expect("Failed to parse schema.json contents");

        debug!("JSON: {:?}", json);

        let input = json
            .get("input")
            .ok_or(Report::new(err2!("Missing input field")))?;

        let output = json
            .get("output")
            .ok_or(Report::new(err2!("Missing output field")))?;

        let convert_required = |required: &Value| -> RResult<bool, AnyErr2> {
            if let Some(required_str) = required.as_str() {
                match required_str {
                    "True" | "true" => Ok(true),
                    "False" | "false" => Ok(false),
                    _ => Err(Report::new(err2!(format!(
                        "Invalid required field string: {:?}",
                        required
                    )))),
                }
            } else if let Some(required_bool) = required.as_bool() {
                Ok(required_bool)
            } else {
                Err(Report::new(err2!(format!(
                    "Invalid required field type: {:?}",
                    required
                ))))
            }
        };

        let convert_params = |params: &Value| -> RResult<Vec<Param>, AnyErr2> {
            debug!("Converting params: {:?}", params);
            let result = params
                .as_array()
                .ok_or(Report::new(err2!(format!(
                    "Expected array, found {:?}",
                    params
                ))))?
                .iter()
                .map(|p| {
                    let mut param_map: serde_json::Map<String, Value> = p
                        .as_object()
                        .ok_or(Report::new(err2!(format!(
                            "Expected object, found {:?}",
                            p
                        ))))?
                        .clone();

                    let required_value =
                        param_map
                            .remove("required")
                            .ok_or(Report::new(err2!(format!(
                                "Missing required field in param: {:?}",
                                p
                            ))))?;
                    let required_bool = convert_required(&required_value)?;
                    param_map.insert("required".to_string(), Value::Bool(required_bool));

                    let param: Param = serde_json::from_value(Value::Object(param_map))
                        .change_context(err2!(format!("Failed to convert param: {:?}", p)))?;

                    Ok(param)
                })
                .collect::<RResult<Vec<Param>, AnyErr2>>();

            debug!("Converted params result: {:?}", result);
            result
        };

        let service_input_params = ServiceInputParams {
            path: input
                .get("path")
                .map_or(Ok(None), |v| convert_params(v).map(Some))?,
            query: input
                .get("query")
                .map_or(Ok(None), |v| convert_params(v).map(Some))?,
            body: input
                .get("body")
                .map_or(Ok(None), |v| convert_params(v).map(Some))?,
        };

        debug!("Service input params: {:?}", service_input_params);

        let convert_output_params = |params: &Value| -> RResult<HashMap<String, Param>, AnyErr2> {
            debug!("Converting output params: {:?}", params);
            let result = params
                .as_array()
                .ok_or(Report::new(err2!(format!(
                    "Expected array, found {:?}",
                    params
                ))))?
                .iter()
                .map(|p| {
                    let mut param_map: serde_json::Map<String, Value> = p
                        .as_object()
                        .ok_or(Report::new(err2!(format!(
                            "Expected object, found {:?}",
                            p
                        ))))?
                        .clone();

                    let required_value =
                        param_map
                            .remove("required")
                            .ok_or(Report::new(err2!(format!(
                                "Missing required field in param: {:?}",
                                p
                            ))))?;

                    let required_bool = convert_required(&required_value)?;
                    param_map.insert("required".to_string(), Value::Bool(required_bool));

                    let param: Param = serde_json::from_value(Value::Object(param_map))
                        .change_context(err2!(format!("Failed to convert param: {:?}", p)))?;

                    Ok((param.name.clone(), param))
                })
                .collect::<RResult<HashMap<String, Param>, AnyErr2>>();

            debug!("Converted output params result: {:?}", result);
            result
        };

        let service_output_params: HashMap<String, Param> = convert_output_params(output)?;

        Ok(ServiceSchema {
            input: service_input_params,
            output: service_output_params,
        })
    }
}
