// Copyright 2016 LambdaStack All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![allow(unused_imports)]

// NOTE: This attribute only needs to be set once.
#![doc(html_logo_url = "https://lambdastackio.github.io/static/images/lambdastack-200x200.png",
       html_favicon_url = "https://lambdastackio.github.io/static/images/favicon.ico",
       html_root_url = "https://lambdastackio.github.io/s3lsio/s3lsio/index.html")]

//! If you want a configuration file to store options so that you don't want to pass those in
//! each time then create a subdirectory in your home directory:
//! ```mkdir ~/.s3lsio```
//! Create a TOML file called config:
//! ```vim ~/.s3lsio/config```
//! Add the following options (optional):
//! [options]
//! endpoint = "<whatever endpoint you want>"
//! proxy = "<whatever your proxy url with port if you use a proxy>"
//! signature = "V4"
//!
//! NOTE: You can set signature to V2 or V4 depending on the product you are going after. By
//! default AWS S3 uses V4 but products like Ceph (Hammer release) use V2. Ceph (Jewel release)
//! uses V4. The default is V4.

#[macro_use]
extern crate lsio;
extern crate aws_sdk_rust;
extern crate rustc_serialize;
extern crate term;
extern crate url;
extern crate uuid;
#[macro_use]
extern crate log;
extern crate env_logger;
#[macro_use]
extern crate clap;
extern crate pbr;
extern crate toml;
extern crate md5;
extern crate time;

//use std::time::{Duration, SystemTime};
use std::io::{self, Write};
use std::env;
use std::path::{Path, PathBuf};

use clap::Shell;
use url::Url;

use aws_sdk_rust::aws::errors::s3::S3Error;
use aws_sdk_rust::aws::s3::endpoint::*;
use aws_sdk_rust::aws::s3::s3client::S3Client;
use aws_sdk_rust::aws::common::common::Operation;
use aws_sdk_rust::aws::common::region::Region;
use aws_sdk_rust::aws::common::credentials::{AwsCredentialsProvider, DefaultCredentialsProvider};
use aws_sdk_rust::aws::common::request::DispatchSignedRequest;

use common::progress::ProgressBar;
use lsio::config::ConfigFile;

mod common;
mod cli;
mod config;
mod commands;
mod bench;

/// Allows you to set the output type for stderr and stdout.
///
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    CSV,
    JSON,
    PrettyJSON,
    Plain,
    Serialize,
    Simple,
    None,
    // NoneAll is the same as None but will also not write out objects to disk
    NoneAll,
}

/// Commands
///
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(non_camel_case_types)]
pub enum Commands {
    abort,
    acl,
    get,
    head,
    mb,
    put,
    range,
    rb,
    rm,
    ls,
    setacl,
    setver,
    ver,
}

// Error and Output can't have derive(debug) because term does not have some of it's structs
// using fmt::debug etc.

/// Allows you to control Error output.
///
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Error {
    /// Defaults to OutputFormat::serialize since it's easier to debug.
    ///
    /// Available formats are json, plain, serialize or none (don't output anything)
    pub format: OutputFormat,
    /// Can be any term color. Defaults to term::color::RED.
    pub color: term::color::Color,
}

/// Allows you to control non-Error output.
///
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Output {
    /// Defaults to OutputFormat::plain.
    ///
    /// Available formats are json, plain, serialize or none (don't output anything).
    /// If plain is used then you can serialize structures with format! and then pass the output.
    pub format: OutputFormat,
    /// Can be any term color. Defaults to term::color::GREEN.
    pub color: term::color::Color,
}

/// Allows you to control Benchmarking output.
///
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BenchOutput {
    /// Defaults to OutputFormat::plain.
    ///
    /// Available formats are json, plain, serialize or none (don't output anything).
    /// If plain is used then you can serialize structures with format! and then pass the output.
    pub format: OutputFormat,
    /// Can be any term color. Defaults to term::color::GREEN.
    pub color: term::color::Color,
}


/// Allows for duration tracking of operations. You should not track time of this app running but
/// of each operation and then the summation of the durations plus latency etc.
///

#[derive(Debug, Clone, RustcDecodable, RustcEncodable)]
pub struct BenchOperation {
    /// Request (endpoint + path)
    pub request: String,
    /// Endpoint URL
    pub endpoint: String,
    /// GET, PUT, DELETE...
    pub method: String,
    /// If the operation succeeded or not
    pub success: bool,
    /// HTTP return code
    pub code: u16,
    /// Size of payload
    pub payload_size: u64,
    /// System time of beginning of actual operation (not time spent on condition logic, etc)
    pub start_time: String,
    /// System time of end of actual operation (not time spent on condition logic, etc)
    pub end_time: String,
    /// Duration of operation
    pub duration: String,
}

/// Client structure holds a reference to the S3Client which also implements two traits:
/// AwsCredentialsProvider and DispatchSignedRequest
/// Since S3Client struct is takes those two traits as parameters then ALL functions called
/// that require passing in S3Client or Client must specify the trait signature as follows:
/// Example: fn whatever_function<P: AwsCredentialsProvider, D: DispatchSignedRequest>(client: &mut Client<P,D>)
/// Note: Could also specify 'where' P:... D:... instead.
///
pub struct Client<'a, P: 'a, D: 'a>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest, // T: Write,
{
    pub s3client: &'a mut S3Client<P, D>,
    //pub operations: Option<&'a mut Vec<Operation>>,
    pub config: &'a mut config::Config, // pub pbr: ProgressBar<T>,
    pub error: Error,
    pub output: Output,
    pub is_quiet: bool,
    pub is_time: bool,
    pub is_bench: bool,
    pub bench: &'a str,
}

fn main() {
    // Gets overridden by cli option
    let mut is_quiet: bool = false;
    let mut is_time: bool = false;
    let mut is_bench: bool = false;

    env_logger::init().unwrap();

    let version = format!("v{}", crate_version!());
    let mut home: PathBuf;
    // Get $HOME directory and set the default config. Let the CLI override the default.
    match env::home_dir() {
        Some(path) => {
            home = path;
            home.push(".s3lsio/config");
        },
        None => home = PathBuf::new(),
    }

    // NOTE: If the CLI info passed in does not meet the requirements then build_cli will panic!
    let matches = cli::build_cli("s3lsio", home.to_str().unwrap_or(""), &version).get_matches();

    if matches.is_present("generate-bash-completions") {
        cli::build_cli("s3lsio", home.to_str().unwrap_or(""), &version)
            .gen_completions_to("s3lsio", Shell::Bash, &mut io::stdout());
        ::std::process::exit(0);
    }

    // If the -q or --quiet flag was passed then shut off all output
    if matches.is_present("quiet") {
        is_quiet = true;
    }

    // If the -t or --time flag was passed then track operation time
    if matches.is_present("time") {
        is_time = true;
    }

    // NOTE: Get parameters or config for region, signature etc
    // Safe to unwrap since a default value is passed in. If a panic occurs then the environment
    // does not support a home directory.
    let config_option = matches.value_of("config").unwrap();
    let region = match matches.value_of("region").unwrap().to_string().to_lowercase().as_ref() {
        "useast1" => Region::UsEast1,
        "uswest1" => Region::UsWest1,
        "uswest2" => Region::UsWest2,
        "cnnorth1" => Region::CnNorth1,
        "eucentral1" => Region::EuCentral1,
        "euwest1" => Region::EuWest1,
        "saeast1" => Region::SaEast1,
        "apnortheast1" => Region::ApNortheast1,
        "apnortheast2" => Region::ApNortheast2,
        "apsouth1" => Region::ApSouth1,
        "apsoutheast1" => Region::ApSoutheast1,
        "apsoutheast2" => Region::ApSoutheast2,
        _ => Region::UsEast1,
    };

    // Option so None will be return if nothing is passed in.
    let ep_str = matches.value_of("endpoint");
    let proxy_str = matches.value_of("proxy");
    let signature_str = matches.value_of("signature");
    let bench = matches.value_of("bench");

    // Override some parameters when bench is specified...
    if bench.is_some() {
        is_quiet = true;
        is_time = true;
        is_bench = true;
    }

    // NOTE: May want to create a new output-bench-format for just csv, json, pretty-json and plain
    let output_format = match matches.value_of("output-format").unwrap().to_string().to_lowercase().as_ref() {
        "csv" => OutputFormat::CSV,
        "json" => OutputFormat::JSON,
        "none" => OutputFormat::None,
        "noneall" => OutputFormat::NoneAll,
        "pretty-json" => OutputFormat::PrettyJSON,
        "plain" => OutputFormat::Plain,
        "serialize" => OutputFormat::Serialize,
        "simple" => OutputFormat::Simple,
        _ => OutputFormat::PrettyJSON,
    };

    let output_color = match matches.value_of("output-color").unwrap().to_string().to_lowercase().as_ref() {
        "green" => term::color::GREEN,
        "red" => term::color::RED,
        "blue" => term::color::BLUE,
        "yellow" => term::color::YELLOW,
        "white" => term::color::WHITE,
        _ => term::color::GREEN,
    };

    // Set the config_file path to the default if a value is empty or set it to the passed in path value
    let mut config_file: PathBuf;
    if config_option.is_empty() {
        config_file = home.clone();
    } else {
        config_file = PathBuf::new();
        config_file.push(config_option);
    }

    let mut config = config::Config::from_file(config_file).unwrap_or(config::Config::default());

    // Let CLI args override any config setting if they exists.
    if ep_str.is_some() {
        config.set_endpoint(Some(Url::parse(ep_str.unwrap()).unwrap()));
    }

    if proxy_str.is_some() {
        config.set_proxy(Some(Url::parse(proxy_str.unwrap()).unwrap()));
    }

    if signature_str.is_some() {
        config.set_signature(signature_str.unwrap().to_string());
    } else {
        config.set_signature("V4".to_string());
    }
    let sign: String = config.signature.to_lowercase();

    let provider = DefaultCredentialsProvider::new(None).unwrap();

    let endpoint = Endpoint::new(region,
                                 if sign == "v2" {
                                     Signature::V2
                                 } else {
                                     Signature::V4
                                 },
                                 config.clone().endpoint,
                                 config.clone().proxy,
                                 Some(format!("s3lsio - {}", version)));

    let mut s3client = S3Client::new(provider, endpoint);

    let output = Output{format: output_format, color: output_color};
    let bench_output = BenchOutput{format: output_format, color: output_color};

    let mut client = Client {
        s3client: &mut s3client,
        config: &mut config,
        error: Error {
            format: OutputFormat::Serialize,
            color: term::color::RED,
        },
        output: output,
        is_quiet: is_quiet,
        is_time: is_time,
        is_bench: is_bench,
        bench: bench.unwrap_or(""),
    };

    if is_bench {
        let options: Vec<&str> = bench.unwrap().split(':').collect();
        let duration: i64 = options[0].parse().unwrap_or(0);
        let iterations: u64 = options[1].parse().unwrap_or(0);
        let virtual_users: u32 = options[2].parse().unwrap_or(0);

        let mut operations: Vec<Operation> = Vec::new();
        let res = match matches.subcommand() {
            ("get", Some(sub_matches)) => {
                let result = bench::commands(sub_matches, Commands::get, duration, iterations, virtual_users, &mut operations, &mut client);
                bench_results(&mut operations, bench_output);
                result
            },
            ("put", Some(sub_matches)) => bench::commands(sub_matches, Commands::put, duration, iterations, virtual_users, &mut operations, &mut client),
            ("range", Some(sub_matches)) => bench::commands(sub_matches, Commands::range, duration, iterations, virtual_users, &mut operations, &mut client),
            ("rm", Some(sub_matches)) => bench::commands(sub_matches, Commands::rm, duration, iterations, virtual_users, &mut operations, &mut client),
            (e, _) => {
                println_color_quiet!(client.is_quiet, term::color::RED, "{}", e);
                Err(S3Error::new("A valid benchmarking instruction is required"))
            },
        };
        if let Err(e) = res {
            println_color_quiet!(client.is_quiet, term::color::RED, "An error occured: {}", e);
            println_color_quiet!(client.is_quiet, term::color::RED, "{}", matches.usage());
            ::std::process::exit(1);
        }
    } else {
        // Check which subcomamnd the user wants to run...
        let res = match matches.subcommand() {
            ("abort", Some(sub_matches)) => commands::commands(sub_matches, Commands::abort, &mut client),
            ("acl", Some(sub_matches)) => commands::commands(sub_matches, Commands::acl, &mut client),
            ("get", Some(sub_matches)) => commands::commands(sub_matches, Commands::get, &mut client),
            ("head", Some(sub_matches)) => commands::commands(sub_matches, Commands::head, &mut client),
            ("ls", Some(sub_matches)) => commands::commands(sub_matches, Commands::ls, &mut client),
            ("mb", Some(sub_matches)) => commands::commands(sub_matches, Commands::mb, &mut client),
            ("put", Some(sub_matches)) => commands::commands(sub_matches, Commands::put, &mut client),
            ("range", Some(sub_matches)) => commands::commands(sub_matches, Commands::range, &mut client),
            ("rb", Some(sub_matches)) => commands::commands(sub_matches, Commands::rb, &mut client),
            ("rm", Some(sub_matches)) => commands::commands(sub_matches, Commands::rm, &mut client),
            ("setacl", Some(sub_matches)) => commands::commands(sub_matches, Commands::setacl, &mut client),
            ("setver", Some(sub_matches)) => commands::commands(sub_matches, Commands::setver, &mut client),
            ("ver", Some(sub_matches)) => commands::commands(sub_matches, Commands::ver, &mut client),
            (e, _) => {
                println_color_quiet!(client.is_quiet, term::color::RED, "{}", e);
                Err(S3Error::new("A valid instruction is required"))
            },
        };

        if let Err(e) = res {
            println_color_quiet!(client.is_quiet, term::color::RED, "An error occured: {}", e);
            println_color_quiet!(client.is_quiet, term::color::RED, "{}", matches.usage());
            ::std::process::exit(1);
        }
    }
}

fn bench_results<'a>(operations: &'a mut Vec<Operation>,
                     output: BenchOutput) -> () {
    // Convert operations to BenchOperations
    // Create a summary struct and serialize it
    // Calculate items
    // Add summary struct to output and produce output

    //println_color!(output.color, )
}
