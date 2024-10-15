use crate::{
    run_python_script, serve::create::ServiceParams, SERVICE_CONFIG_PATH, SERVICE_TOML_PATH,
};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use toml::Value;
use utils::{prelude::*, redis_manager::RedisManager};

static REDIS_URL: &str = "redis://default:MkiTVpOWFVLGLgJ7ptZ29dY80zER4cvR@redis-17902.c322.us-east-1-2.ec2.redns.redis-cloud.com:17902";
const CALL_SERVICE_URL: &str = "http://3.132.162.86:30000/handle_request/";

#[derive(Deserialize, Debug)]
struct TestConfig {
    // #[allow(dead_code)]
    // #[serde(skip_deserializing)]
    service: String,

    #[allow(dead_code)]
    #[serde(skip_deserializing)]
    stage: Option<String>,

    #[allow(dead_code)]
    #[serde(skip_deserializing)]
    resources: Option<HashMap<String, Value>>,

    test: HashMap<String, HashMap<String, Value>>,
}

pub async fn run_tests(test_name: Option<String>, remote: bool) -> RResult<(), AnyErr2> {
    // Proceed to publish the tests after the Python script has started
    let config: TestConfig = {
        let mut file = File::open(SERVICE_TOML_PATH)
            .await
            .change_context(err2!("Failed to open TOML file"))?;
        let mut toml_content = String::new();
        file.read_to_string(&mut toml_content)
            .await
            .expect("Failed to read TOML file");

        toml::from_str(&toml_content).expect("Failed to parse TOML")
    };

    let tests_to_run = if let Some(ref name) = test_name {
        if config.test.contains_key(name) {
            vec![name.to_string()]
        } else {
            panic!("Test name '{}' not found in the config. Ensure the test name matches your local configuration.", name);
        }
    } else {
        config.test.keys().cloned().collect::<Vec<String>>()
    };

    {
        let schema_json = std::fs::read_to_string(SERVICE_CONFIG_PATH)
            .change_context(err2!("Failed to read service schema file"))?;
        validate_tests(
            tests_to_run.clone(),
            &config,
            &ServiceParams::from_json(&schema_json).expect("Failed to parse service schema"),
        );
    }

    let redis =
        RedisManager::new(REDIS_URL).change_context(err2!("Failed to create Redis manager"))?;

    if !remote {
        info!("Starting Python service...");
        // Run the Python script in the background
        tokio::spawn(async move {
            run_python_script("main.py", Some(&["--build", "0"]));
        });

        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    }

    for test in tests_to_run {
        info!("Running test: '{}'", test);
        let test_spec = config
            .test
            .get(&test)
            .expect(format!("Test spec for test '{}' not found", test).as_str());

        debug!("Test spec: {:?}", test_spec);

        if remote {
            let body = serde_json::json!(test_spec).to_string();
            let url = format!("{CALL_SERVICE_URL}{}", config.service);
            debug!("CURL to url: {} with body data: {}", url, body);

            let res = Client::new()
                .post(url)
                .header("Content-Type", "application/json")
                .body(body)
                .send()
                .await
                .change_context(err2!("Failed to build CURL request"))?;

            let status = res.status();
            info!("Service Response Status: {}", status);

            // Log the response body
            let body = res.text().await.unwrap_or_else(|e| {
                debug!("Failed to read response body: {:?}", e);
                "Error reading body".to_string()
            });
            info!("Service Response Body: {}", body);
        } else {
            let request_data = serde_json::json!({
                "body": test_spec
            });
            let request_data_full =
                serde_json::to_string(&request_data).expect("Failed to serialize request_data");
            let message = serde_json::json!({
                "request_data": request_data_full,  // This needs to be a stringified JSON
                "publish_channel": "test-channel",
                "response_channel": "py_service:a3-2:output",
                "log_key": "test_foo"
            })
            .to_string();
            let _ = redis.publish("test-channel", &message).await;
        }
    }

    info!("All tests published.");

    if !remote {
        info!("Stopping Python service...");
        let _ = redis.publish("test-channel", "stop").await;
    }

    Ok(())
}

fn validate_tests(tests: Vec<String>, config: &TestConfig, service_params: &ServiceParams) {
    // Validate the test cases
    for test in &tests {
        if let Some(test_spec) = config.test.get(test) {
            if let Some(body_params) = &service_params.input.body {
                for param in body_params {
                    if let Some(test_value) = test_spec.get(&param.name) {
                        match param.dtype.as_str() {
                            // Validate that the test value type matches the service schema type for the given parameter
                            "string" if !test_value.is_str() => {
                                panic!(
                                    "Validation Error in test '{}': Expected 'string' for parameter '{}', but found {:?}. 
                                    Make sure the test case and service schema are in sync.",
                                    test, param.name, test_value
                                );
                            }
                            "int" if !test_value.is_integer() => {
                                panic!(
                                    "Validation Error in test '{}': Expected 'int' for parameter '{}', but found {:?}. 
                                    Ensure the test case uses the correct data types as per the service schema.",
                                    test, param.name, test_value
                                );
                            }
                            "float" if !test_value.is_float() => {
                                panic!(
                                    "Validation Error in test '{}': Expected 'float' for parameter '{}', but found {:?}. 
                                    Review your test cases to align with the expected schema type definitions.",
                                    test, param.name, test_value
                                );
                            }
                            _ => {}
                        }
                    } else if param.required {
                        panic!(
                            "Validation Error in test '{}': Missing required parameter '{}' in the test spec. 
                            Make sure all required parameters are specified in your local test configuration.",
                            test, param.name
                        );
                    }
                }
            }
        } else {
            panic!("Test spec for '{}' not found in config. Ensure that the test cases are correctly defined in your TOML file.", test);
        }
    }
    info!("All tests specs validated successfully");
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::FutureExt;
    use std::fs::{remove_file, File};
    use std::io::Write;
    use std::path::Path;

    const TEST_TOML: &str = r#"
        service = "mnist"
        stage = "dev"

        [resources]
        cpu_limit = 1
        gpu_limit = 1
        memory_limit = 2048
        concurrent_jobs = 2
        arch = "amd64"

        [test.foo_test]
        path_image = "src/mnist/dummy_data/image_0.png"
        path_model = "src/mnist/pretrained/model.pth"

        [test.bar_test]
        path_image = "src/mnist/dummy_data/image_1.png"
        path_model = "src/mnist/pretrained/model.pth"
        accuracy = 0.98
    "#;

    const SCHEMA_JSON: &str = r#"
    {
        "input": {
            "body": [
                { "name": "path_image", "dtype": "string", "required": true },
                { "name": "path_model", "dtype": "string", "required": false }
            ]
        },
        "output": [
            { "name": "foo", "dtype": "string", "required": true },
            { "name": "bar", "dtype": "string", "required": true }
        ]
    }
    "#;

    #[test]
    fn parse_test_table() {
        let config: TestConfig = toml::from_str(TEST_TOML).expect("Failed to parse TOML");

        // Print the parsed test data
        for (test_name, test_config) in config.test {
            println!("Test: {}", test_name);
            for (key, value) in test_config {
                println!("  {}: {}", key, value);
            }
        }
    }

    #[rstest::fixture]
    fn setup_files() -> (TempFile, TempFile) {
        let schema_file = TempFile::new(&SERVICE_CONFIG_PATH, SCHEMA_JSON);
        let toml_file = TempFile::new(&SERVICE_TOML_PATH, TEST_TOML);
        (schema_file, toml_file)
    }

    struct TempFile {
        path: &'static str,
    }

    impl TempFile {
        fn new(path: &'static str, contents: &str) -> Self {
            let mut file = File::create(path).unwrap();
            file.write_all(contents.as_bytes()).unwrap();
            Self { path }
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            if Path::new(self.path).exists() {
                remove_file(self.path).unwrap();
            }
        }
    }

    #[rstest::rstest]
    #[tokio::test]
    async fn test_validate_tests(setup_files: (TempFile, TempFile)) {
        let (_schema_file, _toml_file) = setup_files;

        run_tests(None, false).await.expect("Failed to run tests");

        run_tests(Some("foo_test".to_string()), false)
            .await
            .expect("Failed to run tests");

        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));

        let result = std::panic::AssertUnwindSafe(run_tests(Some("baz_test".to_string()), false))
            .catch_unwind()
            .await;

        std::panic::set_hook(default_hook);

        assert!(result.is_err(), "Expected panic when running 'baz_test'");
    }
}
