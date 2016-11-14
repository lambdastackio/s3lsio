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

#![feature(plugin)]
#![plugin(clippy)]
#![allow(print_with_newline)]
#![allow(or_fun_call)]

//#![allow(unused_imports)]
#![allow(unused_variables)]

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
extern crate chrono;

use std::io;
use std::io::Write;
use std::env;
use std::path::PathBuf;
use std::fs;
use std::fs::File;
use std::convert::AsRef;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use std::thread;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc;

use clap::{Shell, ArgMatches};
use url::Url;
use rustc_serialize::json;
use chrono::{UTC, DateTime};
use pbr::{ProgressBar, MultiBar};

use aws_sdk_rust::aws::errors::s3::S3Error;
use aws_sdk_rust::aws::s3::endpoint::*;
use aws_sdk_rust::aws::s3::s3client::S3Client;
use aws_sdk_rust::aws::common::common::Operation;
use aws_sdk_rust::aws::common::region::Region;
use aws_sdk_rust::aws::common::credentials::{AwsCredentialsProvider, DefaultCredentialsProviderSync};
use aws_sdk_rust::aws::common::request::DispatchSignedRequest;

use lsio::config::ConfigFile;
use lsio::system::{ip, hostname};

//use progress::ProgressBar;
use common::get_bucket;

mod common;
mod cli;
mod config;
mod commands;
mod bench;

static DEFAULT_USER_AGENT: &'static str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

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
    cp,
    get,
    head,
    keys,  // Admin for Ceph RGW only
    mb,
    put,
    range,
    rb,
    rm,
    ls,
    setacl,
    setver,
    stats, // Admin for Ceph RGW only
    user,  // Admin for Ceph RGW only
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
#[derive(Debug, Default, Clone, RustcEncodable)]
pub struct BenchOperation {
    /// Time operation occured
    //pub time: String,
    pub start_time: String,
    pub end_time: String,
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
    /// Duration of operation
    pub duration: String,
    /// Object name
    pub object: String,
}

/// A summary of all of the operations for a given thread
///
/// start and and end time DO NOT reflect a true duration. ```The total_duration``` does that.
///
#[derive(Debug, Clone, RustcEncodable)]
pub struct BenchThreadSummary {
    // Thread ID/Name
    pub thread_name: String,
    //pub start_time: String,
    // Time the overall benchmarking ended on a given host/instance
    //pub end_time: String,
    pub total_requests: u64,   // Requests are here for compute reasons
    pub total_success: u64,
    pub total_errors: u64,
    pub total_duration: f64,
    pub total_payload: u64,
    pub total_throughput: f64,
    pub operations: Vec<BenchOperation>,
}

impl BenchThreadSummary {
    pub fn new(thread: BenchThread, operations: Vec<BenchOperation>) -> BenchThreadSummary {
        BenchThreadSummary {
            thread_name: thread.thread_name.clone(),
            //start_time: thread.start_time.clone(),
            //end_time: thread.end_time.clone(),
            total_requests: thread.total_requests,
            total_success: thread.total_success,
            total_errors: thread.total_errors,
            total_duration: thread.total_duration,
            total_payload: thread.total_payload,
            total_throughput: thread.total_throughput,
            operations: operations,
        }
    }
}

// Temporary struct to collect totals for a thread before creating the thread summary
// This is here because json encoding does not support &mut Vec...
#[derive(Debug, Default, Clone)]
pub struct BenchThread {
    // Thread ID/Name
    pub thread_name: String,
    //pub start_time: String,
    // Time the overall benchmarking ended on a given host/instance
    //pub end_time: String,
    pub total_requests: u64,   // Requests are here for compute reasons
    pub total_success: u64,
    pub total_errors: u64,
    pub total_duration: f64,
    pub total_payload: u64,
    pub total_throughput: f64,
}

/// A summary of all threads on a given node/instance
///
/// NOTE: The start and end times DO NOT reflect a true duration. They represent the overall time
/// block to execute the operations and compute the results for the given node/instance.
/// ```Total_duration``` is the number of seconds.nanoseconds of actual execution time.
///
/// NOTE: Simple types are used here to make serialization easy.
/// start and and end time DO NOT reflect a true duration. The ```total_duration``` does that.
///
#[derive(Debug, Clone, RustcEncodable)]
pub struct BenchHostInstanceSummary {
    pub host_instance: String,
    pub ip_address: String,
    // Time the benchmarking started on a given host/instance
    pub start_time: String,
    // Time the overall benchmarking ended on a given host/instance
    pub end_time: String,
    pub total_requests: u64,  //Total requests from the sum of the total BenchThreadSummaries
    pub total_success: u64,
    pub total_errors: u64,
    pub total_duration: f64,
    pub total_payload: u64,
    pub total_throughput: f64,
    pub total_threads: u64,
    pub host_duration:f64,
    // Collect metadata of node/vm such as ohai data
    //pub host_instance_metadata: String,
    pub operations: Vec<BenchThreadSummary>
}

impl BenchHostInstanceSummary {
    pub fn new(operations: Vec<BenchThreadSummary>) -> BenchHostInstanceSummary {
        BenchHostInstanceSummary {
            host_instance: "".to_string(),
            ip_address: "".to_string(),
            start_time: "".to_string(),
            end_time: "".to_string(),
            total_requests: 0,
            total_success: 0,
            total_errors: 0,
            total_duration: 0.0,
            total_payload: 0,
            total_throughput: 0.0,
            total_threads: 0,
            host_duration: 0.0,
            operations: operations,
        }
    }
}

/// A summary of all of the hosts used in the benchmarking process.
///
/// start and and end time DO NOT reflect a true duration. The ```total_duration``` does that.
///
#[derive(Debug, Clone, RustcEncodable)]
pub struct BenchSummary {
    // Earliest time benchmarking started on a given host/instance
    pub start_time: String,
    // Time the overall benchmarking ended on a given host/instance
    pub end_time: String,
    pub total_requests: u64,   // Total requests from all of the nodes/instances
    pub total_success: u64,
    pub total_errors: u64,
    pub total_duration: f64,
    pub total_payload: u64,
    pub total_host_instances: u64,
    pub total_threads: u64,    // Total threads that were part of the benchmarking
    pub total_throughput: f64,
    pub operations: Option<Vec<BenchHostInstanceSummary>>,
}

impl BenchSummary {
    pub fn new(operations: Vec<BenchHostInstanceSummary>) -> BenchSummary {
        BenchSummary {
            start_time: "".to_string(),
            end_time: "".to_string(),
            total_requests: 0,
            total_success: 0,
            total_errors: 0,
            total_duration: 0.0,
            total_payload: 0,
            total_host_instances: 0,
            total_threads: 0,
            total_throughput: 0.0,
            operations: Some(operations),
        }
    }
}

/// Metadata for the Benchmarking request.
///
/// iterations - how many iterations to perform. This should be 0 if duration is not 0
/// duration - how many seconds to perform operations. This should be 0 if iterations is not 0
/// ```virtual_users``` - how many simulated users (threads to perform)
/// nodes - how many hosts/VMs to run these operations on
#[derive(Debug, Clone, RustcEncodable)]
pub struct BenchRequest {
    pub date_time: String,
    pub description: String,
    pub endpoint: String,
    pub report: String,
    pub iterations: u64,
    pub duration: u64,
    pub virtual_users: u32,
    pub rampup: u32,
    pub request_type: String,
    pub size: u64,
    pub nodes: u32,
    pub virtual_buckets: bool,
}

/// Allows for duration tracking of operations. You should not track time of this app running but
/// of each operation and then the summation of the durations plus latency etc.
///
#[derive(Debug, Clone, RustcEncodable)]
pub struct BenchResults {
    pub request: BenchRequest,
    pub summary: BenchSummary,
}

impl BenchResults {
    pub fn new(request: BenchRequest, summary: BenchSummary) -> BenchResults {
        BenchResults {
            request: request,
            summary: summary,
        }
    }
}

/// Client structure holds a reference to the ```S3Client``` which also implements two traits:
/// ```AwsCredentialsProvider``` and ```DispatchSignedRequest```
/// Since ```S3Client``` struct is takes those two traits as parameters then ALL functions called
/// that require passing in ```S3Client``` or Client must specify the trait signature as follows:
/// Example: fn ```whatever_function```<P: ```AwsCredentialsProvider```, D: ```DispatchSignedRequest```>(client: &mut Client<P,D>)
/// Note: Could also specify 'where' P:... D:... instead.
///
pub struct Client<'a, P: 'a, D: 'a>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    pub s3client: &'a mut S3Client<P, D>,
    pub config: &'a mut config::Config,
    pub error: Error,
    pub output: Output,
    pub is_quiet: bool,
    pub is_time: bool,
    pub is_bench: bool,
    pub is_admin: bool,
//    pub bench: &'a str,
}

fn main() {
    // Gets overridden by cli option
    let mut is_quiet: bool = false;
    let mut is_time: bool = false;
    let mut is_bench: bool = false;
    let mut is_admin: bool = false;

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

    // If the -a or --admin flag was passed then allow Ceph RGW Admin access (provided keys have rights)
    if matches.is_present("admin") {
        is_admin = true;
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

    let is_bucket_virtual = match matches.value_of("bucket_virtual_host").unwrap().to_string().to_lowercase().as_ref() {
        "false" => false,
        _ => true,
    };

    let output_format = match matches.value_of("output-format").unwrap().to_string().to_lowercase().as_ref() {
        "csv" => OutputFormat::CSV,
        "json" => OutputFormat::JSON,
        "none" => OutputFormat::None,
        "noneall" => OutputFormat::NoneAll,
        "plain" => OutputFormat::Plain,
        "serialize" => OutputFormat::Serialize,
        "simple" => OutputFormat::Simple,
        _ => OutputFormat::PrettyJSON,
    };

    let output_bench_format = match matches.value_of("output-bench-format").unwrap().to_string().to_lowercase().as_ref() {
        "csv" => OutputFormat::CSV,
        "json" => OutputFormat::JSON,
        "plain" => OutputFormat::Plain,
        "serialize" => OutputFormat::Serialize,
        _ => OutputFormat::PrettyJSON,
    };

    let output_color = match matches.value_of("output-color").unwrap().to_string().to_lowercase().as_ref() {
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

    let provider = DefaultCredentialsProviderSync::new(None).unwrap();

    let endpoint = Endpoint::new(region,
                                 if sign == "v2" {
                                     Signature::V2
                                 } else {
                                     Signature::V4
                                 },
                                 config.clone().endpoint,
                                 config.clone().proxy,
                                 Some(DEFAULT_USER_AGENT.to_string()),
                                 Some(is_bucket_virtual));

    let endpoint_clone = endpoint.clone();

    let mut s3client = S3Client::new(provider, endpoint);

    let output = Output{format: output_format, color: output_color};
    let bench_output = BenchOutput{format: output_bench_format, color: output_color};

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
        is_admin: is_admin,
        //bench: bench.unwrap_or(""),
    };

    if is_bench {
        let mut bench_tmp_dir: &str = "";
        let options: Vec<&str> = bench.unwrap().split(':').collect();

        // NB: Duration tests create one s3client per thread while iteration tests create a new s3client per requests per thread.
        // NOTE: Fix - Iterate over this and create defaults...
        let duration: u64 = options[0].parse().unwrap_or(0);
        let iterations: u64 = options[1].parse().unwrap_or(0);
        let virtual_users: u32 = options[2].parse().unwrap_or(0);
        let nodes: u32 = options[3].parse().unwrap_or(1);
        let rampup: u32 = options[4].parse().unwrap_or(0);
        let mut report: &str = options[5];
        if report.is_empty() {
            report = "d"; // Detail
        }
        let report_desc = match report {
            "s" | "S" => "Summary Report",
            _ => "Detail Report",
        };

        // Just default to AWS S3 Standard for now if nothing else
        let ep = match ep_str {
            Some(val) => val,
            _ => "https://s3.amazonaws.com",
        };

        let res = match matches.subcommand() {
            ("get", Some(sub_matches)) => {
                // This part would go into each host instance
                let bench_request = BenchRequest{description: "Benchmarking GET requests...".to_string(),
                                                 date_time: UTC::now().to_string(),
                                                 endpoint: ep.to_string(),
                                                 report: report_desc.to_string(),
                                                 iterations: iterations,
                                                 duration: duration,
                                                 virtual_users: virtual_users,
                                                 request_type: "GET".to_string(),
                                                 rampup: rampup,
                                                 size: 0,
                                                 virtual_buckets: is_bucket_virtual,
                                                 nodes: nodes};
                let bench_host_instance_summary = host_controller(sub_matches, Commands::get, duration, nodes, iterations, virtual_users, 0, endpoint_clone);
                // It would then send the bench_host_instance_summary back to the master and process
                if bench_host_instance_summary.is_some() {
                    master_benchmark(bench_request, bench_output, bench_host_instance_summary.unwrap());
                }
                Ok(())
            },
            ("put", Some(sub_matches)) => {
                let size: u64 = sub_matches.value_of("size").unwrap_or("4096").parse().unwrap_or(4096);
                let bench_request = BenchRequest{description: "Benchmarking PUT requests...".to_string(),
                                                 date_time: UTC::now().to_string(),
                                                 endpoint: ep.to_string(),
                                                 report: report_desc.to_string(),
                                                 iterations: iterations,
                                                 duration: duration,
                                                 virtual_users: virtual_users,
                                                 request_type: "PUT".to_string(),
                                                 rampup: rampup,
                                                 size: size,
                                                 virtual_buckets: is_bucket_virtual,
                                                 nodes: nodes};
                let bench_host_instance_summary = host_controller(sub_matches, Commands::put, duration, nodes, iterations, virtual_users, size, endpoint_clone);
                if bench_host_instance_summary.is_some() {
                    master_benchmark(bench_request, bench_output, bench_host_instance_summary.unwrap());
                }
                Ok(())
            },
            ("gen", Some(sub_matches)) => {
                // NB: Not really needed unless you want to keep files around OR you want to
                // generate a lot files and then shard the put or get requests so that each
                // thread gets or puts a group of files.
                bench_tmp_dir = sub_matches.value_of("path").unwrap_or(".s3lsio_tmp");
                let size: u64 = sub_matches.value_of("size").unwrap_or("4096").parse().unwrap_or(4096);
                let gen_result = gen_files(bench_tmp_dir, "file", iterations, size);
                Ok(())
            }
            ("range", Some(sub_matches)) => {
                let bench_request = BenchRequest{description: "Benchmarking Byte-Range requests...".to_string(),
                                                 date_time: UTC::now().to_string(),
                                                 endpoint: ep.to_string(),
                                                 report: report_desc.to_string(),
                                                 iterations: iterations,
                                                 duration: duration,
                                                 virtual_users: virtual_users,
                                                 request_type: "BYTE-RANGE".to_string(),
                                                 rampup: rampup,
                                                 size: 0,
                                                 virtual_buckets: is_bucket_virtual,
                                                 nodes: nodes};
                let bench_host_instance_summary = host_controller(sub_matches, Commands::range, duration, nodes, iterations, virtual_users, 0, endpoint_clone);
                if bench_host_instance_summary.is_some() {
                    master_benchmark(bench_request, bench_output, bench_host_instance_summary.unwrap());
                }
                Ok(())
            },
            (e, _) => {
                println_color_quiet!(client.is_quiet, term::color::RED, "{}", e);
                Err(S3Error::new("A valid benchmarking instruction is required"))
            },
        };

        // Clean up
        if !bench_tmp_dir.is_empty() {
            let result = fs::remove_dir_all(bench_tmp_dir);
        }

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
            ("cp", Some(sub_matches)) => commands::commands(sub_matches, Commands::cp, &mut client),
            ("head", Some(sub_matches)) => commands::commands(sub_matches, Commands::head, &mut client),
            ("ls", Some(sub_matches)) => commands::commands(sub_matches, Commands::ls, &mut client),
            ("mb", Some(sub_matches)) => commands::commands(sub_matches, Commands::mb, &mut client),
            ("put", Some(sub_matches)) => commands::commands(sub_matches, Commands::put, &mut client),
            ("range", Some(sub_matches)) => commands::commands(sub_matches, Commands::range, &mut client),
            ("rb", Some(sub_matches)) => commands::commands(sub_matches, Commands::rb, &mut client),
            ("rm", Some(sub_matches)) => commands::commands(sub_matches, Commands::rm, &mut client),
            ("setacl", Some(sub_matches)) => commands::commands(sub_matches, Commands::setacl, &mut client),
            ("setver", Some(sub_matches)) => commands::commands(sub_matches, Commands::setver, &mut client),
            ("stats", Some(sub_matches)) => commands::commands(sub_matches, Commands::stats, &mut client),
            ("user", Some(sub_matches)) => commands::commands(sub_matches, Commands::user, &mut client),
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

// NOTE: Will need to refactor this if there are more than one host...
fn master_benchmark(bench_request: BenchRequest,
                    bench_output: BenchOutput,
                    bench_host_instance_summary: BenchHostInstanceSummary) {
    let mut bench_host_instance_operations: Vec<BenchHostInstanceSummary> = Vec::new();
    let mut bench_summary: BenchSummary;

    // This should be called for each host
    bench_host_instance_operations.push(bench_host_instance_summary);
    bench_summary = BenchSummary::new(bench_host_instance_operations);

    // Get results (vec of the hosts results)
    bench_results(bench_request, &mut bench_summary, bench_output);
}

fn host_controller(matches: &ArgMatches,
                   method: Commands,
                   duration: u64,
                   nodes: u32,
                   iterations: u64,
                   virtual_users: u32,
                   size: u64,
                   endpoint: Endpoint) -> Option<BenchHostInstanceSummary>
{
    // Broken out like this since we may want to have a true controller to cause all threads to
    // wait until given the go ahead which will create a thundering heard or create a ramp up
    // controller to be more real world like.

    host_benchmark(matches, method, duration, nodes, iterations, virtual_users, size, endpoint)
}

/*
fn host_controller_trigger(tx: Sender<i32>, millis: u64) {
}
*/

// Runs in the host_controller thread
fn host_benchmark(matches: &ArgMatches,
                  method: Commands,
                  duration: u64,
                  nodes: u32,
                  iterations: u64,
                  virtual_users: u32,
                  size: u64,
                  endpoint: Endpoint) -> Option<BenchHostInstanceSummary>
{
    let duration2 = Duration::from_secs(duration);
    let bench_thread_operations: Vec<BenchThreadSummary> = Vec::new();
    let mut bench_host_instance_summary: BenchHostInstanceSummary;
    let thread_ops_start_times: Vec<DateTime<UTC>> = Vec::new();
    let thread_ops_end_times: Vec<DateTime<UTC>> = Vec::new();

    let mut handles: Vec<_> = Vec::new();

    let arc = Arc::new(Mutex::new(bench_thread_operations));
    let arc_start_times = Arc::new(Mutex::new(thread_ops_start_times));
    let arc_end_times = Arc::new(Mutex::new(thread_ops_end_times));

    let (scheme, tmp_bucket) = matches.value_of("bucket").unwrap_or("s3:// ").split_at(5);
    let mut bucket = get_bucket(tmp_bucket.to_string()).unwrap_or("".to_string());

    if bucket.is_empty() {
        let (scheme, tmp_bucket) = matches.value_of("path").unwrap_or("s3:// ").split_at(5);
        bucket = get_bucket(tmp_bucket.to_string()).unwrap_or("".to_string());
        if bucket.is_empty() {
            println_color_red!("Bucket is empty. Make sure to follow CLI. Issue s3lsio -h for how-to");
            return None;
        }
    }

    // These are only for Byte-Range requests
    let mut offset: u64 = 0;
    let mut len: u64 = 0;

    if method == Commands::range {
        offset = matches.value_of("offset").unwrap_or("0").parse().unwrap_or(0);
        len = matches.value_of("len").unwrap_or("0").parse().unwrap_or(0);
        if len == 0 {
            println_color_red!("Error: Range Len is 0");
            return None;
        }
    }

    let mut pbb = ProgressBar::new(100);

    for i in 0..virtual_users {
        let t_arc = arc.clone();
        let t_arc_start_times = arc_start_times.clone();
        let t_arc_end_times = arc_end_times.clone();
        let t_bucket = bucket.clone();
        let t_endpoint = endpoint.clone();

        pbb.inc();

        // Spawn the threads which represent virtual users
        let handle = thread::spawn(move || {
            let mut operations: Vec<Operation> = Vec::new();
            let thread_name = format!("thread_{:04}", i+1);
            let base_object_name = format!("{}/file{:04}", thread_name, i+1);
            // NB: Each thread has it's own virtual directory in S3 for the given bucket.

            match method {
                Commands::get => {
                    let result = bench::do_get_bench(&t_bucket, &base_object_name, duration2, iterations, None, t_endpoint, &mut operations);
                },
                Commands::put => {
                    let result = bench::do_put_bench(&t_bucket, &base_object_name, duration2, iterations, size, t_endpoint, &mut operations);
                },
                Commands::range => {
                    let range = format!("bytes={}-{}", offset, len);
                    let result = bench::do_get_bench(&t_bucket, &base_object_name, duration2, iterations, Some(&range), t_endpoint, &mut operations);
                },
                _ => {},
            }

            // start - The earliest operation of the given thread
            // end - The latest operation of the given thread
            let (bench_thread, bench_operations, start, end) = bench_thread_results(&operations);

            let mut bench_thread_summary = BenchThreadSummary::new(bench_thread, bench_operations);
            bench_thread_summary.thread_name = thread_name.clone();

            let mut bto = t_arc.lock().unwrap();
            bto.push(bench_thread_summary);

            let mut start_times = t_arc_start_times.lock().unwrap();
            let mut end_times = t_arc_end_times.lock().unwrap();
            start_times.push(start);
            end_times.push(end);
        });

        handles.push(handle);
    }

    // Wait on above threads to complete before going on...
    for handle in handles {
        handle.join().unwrap();
    }

    // NOTE: Get the data from Mutex and clone it to create a "new" ownership that can be added
    // to the collections below...
    let bto_mutex = arc.lock().unwrap();
    let mut bto: Vec<BenchThreadSummary> = Vec::new();

    for b in bto_mutex.iter() {
        let nb = b.clone();
        bto.push(nb.clone());
    }

    bench_host_instance_summary = BenchHostInstanceSummary::new(bto);
    bench_host_instance_summary.host_instance = hostname().unwrap_or("".to_string());
    bench_host_instance_summary.ip_address = ip("").unwrap().to_string();

    let start_time_mutex = arc_start_times.lock().unwrap();
    let mut start_time: DateTime<UTC> = UTC::now();

    for s in start_time_mutex.iter() {
        let st = *s; //s.clone();
        if st <= start_time {
            start_time = st;
        }
    }

    let end_time_mutex = arc_end_times.lock().unwrap();
    let mut end_time: DateTime<UTC> = UTC::now();

    for e in end_time_mutex.iter() {
        let et = *e; //e.clone();
        if et >= end_time {
            end_time = et;
        }
    }

    bench_host_instance_summary.start_time = start_time.format("%Y-%m-%d %H:%M:%S%.9f %z").to_string();
    bench_host_instance_summary.end_time = end_time.format("%Y-%m-%d %H:%M:%S%.9f %z").to_string();

    let total_duration: Duration = (end_time - start_time).to_std().unwrap();
    let duration_str: String = format!("{}.{}", total_duration.as_secs(), total_duration.subsec_nanos());
    let duration: f64 =  duration_str.parse::<f64>().unwrap() as f64;

    bench_host_instance_summary.host_duration = duration;

    // Get the earliest start_time and latest end_time of all of the threads for the given host.
    // This is used to determine true throughput for host. This data will then go to a collector
    // that runs the stats for all hosts before presenting final results.

    // NB: Only one host for now...

    // Get host results (vec of the thread results)
    bench_host_instance_results(&mut bench_host_instance_summary);

    // Pass the bench_host_instance_summary of each host back to the master/primary
    // and add them to bench_host_instance_operations

    pbb.finish();

    Some(bench_host_instance_summary)
}

// Use this function if you want to generate a number of actual files of a given size with a given
// prefix (i.e. 'file').
fn gen_files(tmp_dir: &str, base_object_name: &str, iterations: u64, size: u64) -> Result<(), S3Error> {
    // Remove the tmp directory .s3lsio_tmp
    let result = fs::remove_dir_all(tmp_dir);
    fs::create_dir_all(tmp_dir).unwrap();

    let mut object: String;
    let path: String = format!("{}{}", tmp_dir, if tmp_dir.ends_with('/') {""} else {"/"});

    for i in 0..iterations {
        object = format!("{}{}{}", path, base_object_name, i);
        {
            match File::create(object) {
                Ok(f) => {
                    let result_len = f.set_len(size);
                },
                Err(e) => {
                    let error = format!("{:#?}", e);
                    println_color!(term::color::RED, "{}", error);
                    return Err(S3Error::new(error));
                },
            }
        }
    }

    Ok(())
}

// Moves the Vec Operations into BenchOperations and adds them to thread_summary
//fn bench_thread_results(operations: &Vec<Operation>) -> (BenchThread, Vec<BenchOperation>, DateTime<UTC>, DateTime<UTC>) {
fn bench_thread_results(operations: &[Operation]) -> (BenchThread, Vec<BenchOperation>, DateTime<UTC>, DateTime<UTC>) {
    let mut total_errors: u64 = 0;
    //let mut total_duration: f64 = 0.0;
    let mut total_duration = Duration::new(0,0);
    let mut total_payload: u64 = 0;

    let mut bench_operations: Vec<BenchOperation> = Vec::with_capacity(operations.len());
    let mut bench_thread_summary = BenchThread::default();

    let mut start: DateTime<UTC> = UTC::now();
    let mut end: DateTime<UTC> = UTC::now();

    bench_thread_summary.total_requests = operations.len() as u64;

    for op in operations {
        let mut bop = BenchOperation::default();
        let duration_str: String = format!("{}.{}", op.duration.unwrap().as_secs(), op.duration.unwrap().subsec_nanos());
        let duration = op.duration.unwrap();

        bop.request = op.request.clone();
        bop.endpoint = op.endpoint.clone();
        bop.method = op.method.clone();
        bop.success = op.success;
        bop.code = op.code;
        bop.payload_size = op.payload_size;
        bop.duration = duration_str.clone();
        if op.object.starts_with('/') {
            bop.object = op.object.clone()[1..].to_string();
        } else {
            bop.object = op.object.clone();
        }

        bop.start_time = op.start_time.unwrap().format("%Y-%m-%d %H:%M:%S%.9f %z").to_string();
        bop.end_time = op.end_time.unwrap().format("%Y-%m-%d %H:%M:%S%.9f %z").to_string();

        if op.start_time.unwrap() <= start {
            start = op.start_time.unwrap();
        }

        if op.end_time.unwrap() >= end {
            end = op.end_time.unwrap();
        }

        total_duration += duration;
        total_payload += op.payload_size;

        if !op.success {
            total_errors += 1;
        }

        bench_operations.push(bop);
    }

    let duration_str: String = format!("{}.{}", total_duration.as_secs(), total_duration.subsec_nanos());
    let duration: f64 =  duration_str.parse::<f64>().unwrap() as f64;

    bench_thread_summary.total_duration = duration;
    bench_thread_summary.total_errors = total_errors;
    bench_thread_summary.total_payload = total_payload;
    bench_thread_summary.total_success = bench_thread_summary.total_requests - bench_thread_summary.total_errors;
    bench_thread_summary.total_throughput = (bench_thread_summary.total_success as f64 / duration) as f64;

    (bench_thread_summary, bench_operations, start, end)
}

// Rolls up all of the thread summaries for a given host
fn bench_host_instance_results(bench_host_instance_summary: &mut BenchHostInstanceSummary) -> () {
    let mut total_errors: u64 = 0;
    let mut total_duration: f64 = 0.0;
    let mut total_payload: u64 = 0;
    let mut total_threads: u64 = 0;
    let mut total_success: u64 = 0;
    let mut total_requests: u64 = 0;

    let operations = bench_host_instance_summary.clone();

    // Just rolling up totals...
    for op in operations.operations {
        total_duration += op.total_duration;
        total_payload += op.total_payload;
        total_errors += op.total_errors;
        total_success += op.total_success;
        total_requests += op.total_requests;

        total_threads += 1;
    }

    bench_host_instance_summary.total_duration = total_duration;
    bench_host_instance_summary.total_errors = total_errors;
    bench_host_instance_summary.total_payload = total_payload;
    bench_host_instance_summary.total_threads = total_threads;
    bench_host_instance_summary.total_success = total_success;
    bench_host_instance_summary.total_requests = total_requests;
    bench_host_instance_summary.total_throughput = (total_success as f64 / total_duration) as f64;
}

// Rolls up the hosts for a summary... For now there is only one hosts...
fn bench_results(metadata: BenchRequest,
                 bench_summary: &mut BenchSummary,
                 output: BenchOutput) -> () {
    let mut total_errors: u64 = 0;
    let mut total_duration: f64 = 0.0;
    let mut total_payload: u64 = 0;
    let mut total_host_instances: u64 = 0;
    let mut total_threads: u64 = 0;
    let mut total_success: u64 = 0;
    let mut total_requests: u64 = 0;

    let operations = bench_summary.clone();

    // Just rolling up totals...
    for op in operations.operations.unwrap() {
        total_duration += op.total_duration;
        total_payload += op.total_payload;
        total_errors += op.total_errors;
        total_success += op.total_success;
        total_requests += op.total_requests;
        total_threads += op.total_threads;

        total_host_instances += 1;
    }

    // Only has one host for now so we can cheat :)
    let summary = bench_summary.clone().operations.unwrap();
    bench_summary.start_time = summary[0].start_time.clone();
    bench_summary.end_time = summary[0].end_time.clone();

    // Looks at the earliest thread start time and the latest end time and then recomputes totals
    let start_time = DateTime::parse_from_str(&bench_summary.start_time, "%Y-%m-%d %H:%M:%S%.9f %z");
    let end_time = DateTime::parse_from_str(&bench_summary.end_time, "%Y-%m-%d %H:%M:%S%.9f %z");

    let dur: Duration = (end_time.unwrap() - start_time.unwrap()).to_std().unwrap();
    let duration_str: String = format!("{}.{}", dur.as_secs(), dur.subsec_nanos());
    let duration: f64 =  duration_str.parse::<f64>().unwrap() as f64;

    // Truncates off the decimal portion for duration tests
    if metadata.iterations == 0 {
        bench_summary.total_duration = duration.trunc();
    } else {
        bench_summary.total_duration = duration;
    }
    bench_summary.total_errors = total_errors;
    bench_summary.total_payload = total_payload;
    bench_summary.total_threads = total_threads;
    bench_summary.total_host_instances = total_host_instances;
    bench_summary.total_success = total_success;
    bench_summary.total_requests = total_requests;
    // Only successful requests are used in throughput
    bench_summary.total_throughput = (total_success as f64 / duration) as f64;

    // NB: *If the report type contains summary then make bench_summary.operations = None
    if metadata.report.contains("Summary") {
        bench_summary.operations = None;
    }

    let bench_results = BenchResults::new(metadata, bench_summary.clone());

    match output.format {
        OutputFormat::JSON => {
            println_color!(output.color,
                                 "{}",
                                 json::encode(&bench_results).unwrap_or("{}".to_string()));
        },
        OutputFormat::PrettyJSON => {
            println_color!(output.color, "{}", json::as_pretty_json(&bench_results));
        },
        _ => {
            println_color!(output.color, "{:#?}", bench_results);
        },
    }
}
