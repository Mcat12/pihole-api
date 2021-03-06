// Pi-hole: A black hole for Internet advertisements
// (c) 2019 Pi-hole, LLC (https://pi-hole.net)
// Network-wide ad blocking via your own hardware.
//
// API
// Common Test Functions
//
// This file is copyright under the latest version of the EUPL.
// Please see LICENSE file for your rights under this license.

use crate::{
    databases::{
        create_memory_db,
        ftl::{FtlDatabase, FtlDatabasePool, FtlDatabasePoolParameters, TEST_FTL_DATABASE_SCHEMA},
        gravity::{
            GravityDatabase, GravityDatabasePool, GravityDatabasePoolParameters,
            TEST_GRAVITY_DATABASE_SCHEMA,
        },
        DatabaseService, FakeDatabaseService,
    },
    env::{Config, Env, PiholeFile},
    ftl::{FtlConnectionType, FtlCounters, FtlMemory, FtlSettings},
    services::PiholeModule,
    setup,
};
use rocket::{
    http::{ContentType, Header, Method, Status},
    local::blocking::Client,
};
use shaku::{HasComponent, HasProvider, Interface, ModuleBuilder, ProviderFn};
use std::{
    collections::HashMap,
    fs::File,
    io::{prelude::*, SeekFrom},
};
use tempfile::NamedTempFile;

/// Add the end of message byte to the data
pub fn write_eom(data: &mut Vec<u8>) {
    data.push(0xc1);
}

/// Builds the data needed to create a `Env::Test`
pub struct TestEnvBuilder {
    test_files: Vec<TestFile<NamedTempFile>>,
}

impl TestEnvBuilder {
    /// Create a new `TestEnvBuilder`
    pub fn new() -> TestEnvBuilder {
        TestEnvBuilder {
            test_files: Vec::new(),
        }
    }

    /// Add a file and verify that it does not change
    pub fn file(self, pihole_file: PiholeFile, initial_data: &str) -> Self {
        self.file_expect(pihole_file, initial_data, initial_data)
    }

    /// Add a file and verify that it ends up in a certain state
    pub fn file_expect(
        mut self,
        pihole_file: PiholeFile,
        initial_data: &str,
        expected_data: &str,
    ) -> Self {
        let test_file = TestFile::new(
            pihole_file,
            NamedTempFile::new().unwrap(),
            initial_data.to_owned(),
            expected_data.to_owned(),
        );
        self.test_files.push(test_file);
        self
    }

    /// Build the environment. This will create an `Env::Test` with a default
    /// config.
    pub fn build(self) -> Env {
        let mut env_data = HashMap::new();

        // Create temporary mock files
        for mut test_file in self.test_files {
            // Write the initial data to the file
            write!(test_file.temp_file, "{}", test_file.initial_data).unwrap();
            test_file.temp_file.seek(SeekFrom::Start(0)).unwrap();

            // Save the file for the test
            env_data.insert(test_file.pihole_file, test_file.temp_file);
        }

        Env::Test(Config::default(), env_data)
    }

    /// Get a copy of the inner test files for later verification
    pub fn clone_test_files(&self) -> Vec<TestFile<File>> {
        let mut test_files = Vec::new();

        for test_file in &self.test_files {
            test_files.push(TestFile {
                pihole_file: test_file.pihole_file,
                temp_file: test_file.temp_file.reopen().unwrap(),
                initial_data: test_file.initial_data.clone(),
                expected_data: test_file.expected_data.clone(),
            })
        }

        test_files
    }
}

/// Represents a mocked file along with the initial and expected data. The `T`
/// generic is the type of temporary file, usually `NamedTempFile` or `File`.
pub struct TestFile<T: Seek + Read> {
    pihole_file: PiholeFile,
    temp_file: T,
    initial_data: String,
    expected_data: String,
}

impl<T: Seek + Read> TestFile<T> {
    /// Create a new `TestFile`
    fn new(
        pihole_file: PiholeFile,
        temp_file: T,
        initial_data: String,
        expected_data: String,
    ) -> TestFile<T> {
        TestFile {
            pihole_file,
            temp_file,
            initial_data,
            expected_data,
        }
    }

    /// Asserts that the contents of the file matches the expected contents.
    /// `buffer` is used to read the file into memory, and will be cleared at
    /// the end.
    pub fn assert_expected(&mut self, buffer: &mut String) {
        self.temp_file.seek(SeekFrom::Start(0)).unwrap();
        self.temp_file.read_to_string(buffer).unwrap();

        assert_eq!(*buffer, self.expected_data);
        buffer.clear();
    }
}

/// Represents a test configuration, with all the data needed to carry out the
/// test
pub struct TestBuilder {
    endpoint: String,
    method: Method,
    headers: Vec<Header<'static>>,
    should_auth: bool,
    auth_required: bool,
    body_data: Option<serde_json::Value>,
    ftl_data: HashMap<String, Vec<u8>>,
    ftl_memory: FtlMemory,
    test_env_builder: TestEnvBuilder,
    expected_json: serde_json::Value,
    expected_status: Status,
    needs_database: bool,
    module_builder: ModuleBuilder<PiholeModule>,
}

impl TestBuilder {
    pub fn new() -> TestBuilder {
        TestBuilder {
            endpoint: "".to_owned(),
            method: Method::Get,
            headers: Vec::new(),
            should_auth: true,
            auth_required: true,
            body_data: None,
            ftl_data: HashMap::new(),
            ftl_memory: FtlMemory::Test {
                clients: Vec::new(),
                domains: Vec::new(),
                over_time: Vec::new(),
                queries: Vec::new(),
                upstreams: Vec::new(),
                strings: HashMap::new(),
                counters: FtlCounters::default(),
                settings: FtlSettings::default(),
            },
            test_env_builder: TestEnvBuilder::new(),
            expected_json: json!({
                "data": [],
                "errors": []
            }),
            expected_status: Status::Ok,
            needs_database: false,
            module_builder: PiholeModule::builder(),
        }
    }

    pub fn endpoint(mut self, endpoint: &str) -> Self {
        self.endpoint = endpoint.to_owned();
        self
    }

    pub fn method(mut self, method: Method) -> Self {
        self.method = method;
        self
    }

    pub fn header<H: Into<Header<'static>>>(mut self, header: H) -> Self {
        self.headers.push(header.into());
        self
    }

    pub fn should_auth(mut self, should_auth: bool) -> Self {
        self.should_auth = should_auth;
        self
    }

    /// If the server requires authentication for protected routes
    pub fn auth_required(mut self, auth_required: bool) -> Self {
        self.auth_required = auth_required;
        self
    }

    pub fn body<T: Into<serde_json::Value>>(mut self, body: T) -> Self {
        self.body_data = Some(body.into());
        self
    }

    pub fn ftl(mut self, command: &str, data: Vec<u8>) -> Self {
        self.ftl_data.insert(command.to_owned(), data);
        self
    }

    pub fn ftl_memory(mut self, ftl_memory: FtlMemory) -> Self {
        self.ftl_memory = ftl_memory;
        self
    }

    pub fn file(mut self, pihole_file: PiholeFile, initial_data: &str) -> Self {
        self.test_env_builder = self.test_env_builder.file(pihole_file, initial_data);
        self
    }

    pub fn file_expect(
        mut self,
        pihole_file: PiholeFile,
        initial_data: &str,
        expected_data: &str,
    ) -> Self {
        self.test_env_builder =
            self.test_env_builder
                .file_expect(pihole_file, initial_data, expected_data);
        self
    }

    pub fn expect_json<T: Into<serde_json::Value>>(mut self, expected_json: T) -> Self {
        self.expected_json = expected_json.into();
        self
    }

    pub fn expect_status(mut self, status: Status) -> Self {
        self.expected_status = status;
        self
    }

    // This method is not used for now, but could be in the the future
    #[allow(unused)]
    pub fn need_database(mut self, need_database: bool) -> Self {
        self.needs_database = need_database;
        self
    }

    #[allow(unused)]
    pub fn mock_component<I: Interface + ?Sized>(mut self, component: Box<I>) -> Self
    where
        PiholeModule: HasComponent<I>,
    {
        self.module_builder = self.module_builder.with_component_override(component);
        self
    }

    pub fn mock_provider<I: ?Sized + 'static>(
        mut self,
        provider_fn: ProviderFn<PiholeModule, I>,
    ) -> Self
    where
        PiholeModule: HasProvider<I>,
    {
        self.module_builder = self.module_builder.with_provider_override(provider_fn);
        self
    }

    pub fn test(mut self) {
        // Save the files for verification
        let test_files = self.test_env_builder.clone_test_files();

        let api_key = if self.auth_required {
            Some("test_key".to_owned())
        } else {
            None
        };
        let env = self.test_env_builder.build();
        let config = env.config().clone();

        // Configure the module
        self.module_builder = self
            .module_builder
            .with_component_parameters::<Env>(env)
            .with_component_parameters::<FtlConnectionType>(
            FtlConnectionType::Test(self.ftl_data),
        );

        self.module_builder = if self.needs_database {
            self.module_builder
                .with_component_parameters::<GravityDatabasePool>(GravityDatabasePoolParameters {
                    pool: create_memory_db(TEST_GRAVITY_DATABASE_SCHEMA, 1),
                })
                .with_component_parameters::<FtlDatabasePool>(FtlDatabasePoolParameters {
                    pool: create_memory_db(TEST_FTL_DATABASE_SCHEMA, 1),
                })
        } else {
            self.module_builder
                .with_component_override::<dyn DatabaseService<GravityDatabase>>(Box::new(
                    FakeDatabaseService,
                ))
                .with_component_override::<dyn DatabaseService<FtlDatabase>>(Box::new(
                    FakeDatabaseService,
                ))
        };

        // Configure the test server
        let rocket = setup::test(
            self.ftl_memory,
            &config,
            api_key,
            self.module_builder.build(),
        );

        // Start the test client
        let client = Client::untracked(rocket).unwrap();

        // Create the request
        let mut request = client.req(self.method, self.endpoint);

        // Add the authentication header
        if self.should_auth {
            request.add_header(Header::new("X-Pi-hole-Authenticate", "test_key"));
        }

        // Add the rest of the headers
        for header in self.headers {
            request.add_header(header);
        }

        // Set the body data if necessary
        if let Some(data) = self.body_data {
            request.add_header(ContentType::JSON);
            request.set_body(serde_json::to_vec(&data).unwrap());
        }

        // Dispatch the request
        println!("{:#?}", request);
        let response = request.dispatch();
        println!("\nResponse:\n{:?}", response);

        // Check the status
        assert_eq!(self.expected_status, response.status());

        // Check that something was returned
        let body = response.into_string();
        assert!(body.is_some());

        let body_str = body.unwrap();
        println!("Body:\n{}", body_str);

        // Check that it is correct JSON
        let parsed: serde_json::Value = serde_json::from_str(&body_str).unwrap();

        // Check that is is the same as the expected JSON
        assert_eq!(self.expected_json, parsed);

        // Check the files against the expected data
        let mut buffer = String::new();
        for mut test_file in test_files {
            test_file.assert_expected(&mut buffer);
        }
    }
}
