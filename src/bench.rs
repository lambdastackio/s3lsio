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
#![allow(unused_must_use)]
#![allow(unused_variables)]

use std::io;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::fs;
use std::fs::File;
use std::ffi::OsStr;
use std::time::{Duration, Instant};

use std::sync::{Arc, Mutex};
use std::thread;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc;

use md5;
use term;
use rustc_serialize::json;
use rustc_serialize::base64::{STANDARD, ToBase64};

use chrono::{UTC, DateTime};

use clap::ArgMatches;
use aws_sdk_rust::aws::errors::s3::S3Error;
//use aws_sdk_rust::aws::common::credentials::AwsCredentialsProvider;
use aws_sdk_rust::aws::s3::s3client::S3Client;
use aws_sdk_rust::aws::s3::endpoint::*;

use aws_sdk_rust::aws::common::credentials::{AwsCredentialsProvider, DefaultCredentialsProviderSync};
use aws_sdk_rust::aws::common::region::Region;

use aws_sdk_rust::aws::common::request::DispatchSignedRequest;
use aws_sdk_rust::aws::common::common::Operation;
use aws_sdk_rust::aws::s3::acl::*;
use aws_sdk_rust::aws::s3::bucket::*;
use aws_sdk_rust::aws::s3::object::*;

use lsio::system::{ip, hostname};

use Client;
use Output;
use OutputFormat;
use Commands;
use common::get_bucket;

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

pub fn benchmarking<'a, P, D>(matches: ArgMatches,
                              bench: Option<&str>,
                              ep_str: Option<&str>,
                              is_bucket_virtual: bool,
                              bench_output: BenchOutput,
                              client: Client<P, D>)
                              where P: AwsCredentialsProvider + Sync + Send,
                                    D: DispatchSignedRequest + Sync + Send,
{
    let endpoint_clone = client.s3client.endpoint().clone();
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
}

/// Benchmarking - This benchmarking is for the server only and not the app. Thus, only the
/// last library layer (hyper) is measured and not any local disk activity so that a more accurate
/// measurement can be taken.
///
/// Benchmarks do not write the data to disk after GETs like a normal operation does. It does
/// however create synthetic data in a temporary directory that is specified.
///
pub fn commands<'a, P, D>(matches: &ArgMatches,
                              cmd: Commands,
                              duration: Duration,
                              iterations: u64,
                              virtual_users: u32,
                              len: u64,
                              base_object_name: &str,
                              operations: &'a mut Vec<Operation>,
                              client: &Client<P, D>) -> Result<(), S3Error>
                              where P: AwsCredentialsProvider + Sync + Send,
                                    D: DispatchSignedRequest + Sync + Send,
{
    let mut bucket: &str = "";
    //let mut object: String = "".to_string();
    // Make sure the s3 schema prefix is present
    let (scheme, tmp_bucket) = matches.value_of("bucket").unwrap_or("s3:// ").split_at(5);

    if tmp_bucket.contains('/') {
        let components: Vec<&str> = tmp_bucket.split('/').collect();
        let mut first: bool = true;
        //let mut object_first: bool = true;

        for part in components {
            if first {
                bucket = part;
                break;
            } //else {
            //    if !object_first {
            //        object += "/";
            //    }
            //    object_first = false;
            //    object += part;
            //}
            first = false;
        }
    } else {
        bucket = tmp_bucket.trim();
    }

    // NOTE: Change the User-Agent to mean something in the logs
    // S3lsio Benchmarking : <IP Address> : <User ???? - not sure if we want this>
    // Separate each segment with : so that the logs can be easily grepped and awked etc.

    match cmd {
        Commands::get => {
            let result = get_bench(bucket, base_object_name, &duration, iterations, operations, client);
        },
        Commands::put => {
            let path = matches.value_of("path").unwrap_or("");
            let result = put_bench(bucket, path, base_object_name, &duration, iterations, len, operations, client);
        },
        Commands::range => {
            let offset: u64 = matches.value_of("offset").unwrap_or("0").parse().unwrap_or(0);
            let len: u64 = matches.value_of("len").unwrap_or("0").parse().unwrap_or(0);
            if len == 0 {
                println_color_quiet!(client.is_quiet, client.error.color, "Error Byte-Range request: Len must be > 0");
                return Err(S3Error::new("Error Byte-Range request: Len must be > 0"));
            }
            let result = get_range_bench(bucket, base_object_name, duration, iterations, offset, len, operations, client);
        },
        _ => {}
    }

    Ok(())
}

fn get_range_bench<'a, P, D>(bucket: &str,
                                 base_object_name: &str,
                                 duration: Duration,
                                 iterations: u64,
                                 offset: u64,
                                 len: u64,
                                 operations: &'a mut Vec<Operation>,
                                 client: &Client<P, D>) -> Result<(), S3Error>
                                 where P: AwsCredentialsProvider,
                                       D: DispatchSignedRequest,
{
    let mut object: String;

    if iterations > 0 {
        for i in 0..iterations {
            let mut operation: Operation;
            operation = Operation::default();
            object = format!("{}{:04}", base_object_name, i+1);
            let result = get_object_range(bucket, &object, offset, len, Some(&mut operation), client);
            operations.push(operation);
        }
    } else if duration.as_secs() > 0 {
        let mut count: u64 = 0;
        let now = Instant::now();

        loop {
            let mut operation: Operation;
            operation = Operation::default();
            object = format!("{}{:04}", base_object_name, count+1);
            let result = get_object_range(bucket, &object, offset, len, Some(&mut operation), client);
            operations.push(operation);
            //if now.elapsed() >= *duration {
            if now.elapsed() >= duration {
                break;
            }
            count += 1;
        }
    }

    Ok(())
}

fn get_bench<'a, 'b, P, D>(bucket: &str,
                           base_object_name: &str,
                           duration: &'b Duration,
                           iterations: u64,
                           operations: &'a mut Vec<Operation>,
                           client: &Client<P, D>) -> Result<(), S3Error>
                           where P: AwsCredentialsProvider,
                                 D: DispatchSignedRequest,
{
    let mut object: String;

    if iterations > 0 {
        for i in 0..iterations {
            let mut operation: Operation;
            operation = Operation::default();
            object = format!("{}{:04}", base_object_name, i+1);
            let result = get_object(bucket, &object, Some(&mut operation), client);
            operations.push(operation);
        }
    } else if duration.as_secs() > 0 {
        let mut count: u64 = 0;
        let now = Instant::now();

        loop {
            let mut operation: Operation;
            operation = Operation::default();
            object = format!("{}{:04}", base_object_name, count+1);
            let result = get_object(bucket, &object, Some(&mut operation), client);
            operations.push(operation);
            if now.elapsed() >= *duration {
                break;
            }
            count += 1;
        }
    }

    Ok(())
}

pub fn do_get_bench<'a>(bucket: &str,
                        base_object_name: &str,
                        duration: Duration,
                        iterations: u64,
                        range: Option<&'a str>,
                        endpoint: Endpoint,
                        operations: &'a mut Vec<Operation>) -> Result<(), S3Error>
{
    let mut object: String;

    // NB: For iterations we allocate new s3client each time to simulate single user transactions...
    if iterations > 0 {
        for i in 0..iterations {
            let mut operation = Operation::default();

            // NB: For benchmarking, the objects are synthetic and in a predictable naming format.
            object = format!("{}{:04}", base_object_name, i+1);

            let mut request = GetObjectRequest::default();
            request.bucket = bucket.to_string();
            request.key = object.clone();
            if range.is_some() {
                request.range = Some(range.unwrap().clone().to_string());
            }

            let provider = DefaultCredentialsProviderSync::new(None).unwrap();
            let local_endpoint = endpoint.clone();
            let s3client = S3Client::new(provider, local_endpoint);

            match s3client.get_object(&request, Some(&mut operation)) {
                Ok(output) => {},
                Err(e) => {
                    println_color_red!("Failed to get [{}/{}] - {}", bucket, object, e);
                },
            }

            operations.push(operation);
        }
    } else if duration.as_secs() > 0 {
        let mut count: u64 = 0;
        let now = Instant::now();

        let provider = DefaultCredentialsProviderSync::new(None).unwrap();
        let local_endpoint = endpoint.clone();
        let s3client = S3Client::new(provider, local_endpoint);

        // NOTE: Don't need to move the GetObjectRequest to the loop like you do on put object...
        let mut request = GetObjectRequest::default();
        request.bucket = bucket.to_string();
        if range.is_some() {
            request.range = Some(range.unwrap().clone().to_string());
        }

        loop {
            let mut operation = Operation::default();
            object = format!("{}{:04}", base_object_name, count+1);

            request.key = object.clone();

            match s3client.get_object(&request, Some(&mut operation)) {
                Ok(output) => {},
                Err(e) => {
                    println_color_red!("Failed to get [{}/{}] - {}", bucket, object, e);
                },
            }

            if now.elapsed() >= duration {
                operations.push(operation);
                break;
            }

            operations.push(operation);
            count += 1;
        }
    }

    Ok(())
}

pub fn do_put_bench<'a>(bucket: &str,
                        base_object_name: &str,
                        duration: Duration,
                        iterations: u64,
                        size: u64,
                        endpoint: Endpoint,
                        operations: &'a mut Vec<Operation>) -> Result<(), S3Error>
{
    let mut object: String;

    // NB: For iterations we allocate new s3client each time to simulate single user transactions...
    if iterations > 0 {
        for i in 0..iterations {
            let mut operation = Operation::default();

            // NB: For benchmarking, the objects are synthetic and in a predictable naming format.
            object = format!("{}{:04}", base_object_name, i+1);

            // Synthetic buffer creation to simulate an on disk object
            let mut buffer: Vec<u8>;
            zero_fill_buffer!(buffer, size);

            let mut request = PutObjectRequest::default();
            request.bucket = bucket.to_string();
            request.key = object.clone();
            request.body = Some(&buffer);

            let provider = DefaultCredentialsProviderSync::new(None).unwrap();
            let local_endpoint = endpoint.clone();
            let s3client = S3Client::new(provider, local_endpoint);

            match s3client.put_object(&request, Some(&mut operation)) {
                Ok(output) => {},
                Err(e) => {
                    println_color_red!("Failed to put [{}/{}] - {}", bucket, object, e);
                },
            }

            operations.push(operation);
        }
    } else if duration.as_secs() > 0 {
        let mut count: u64 = 0;
        let now = Instant::now();

        let provider = DefaultCredentialsProviderSync::new(None).unwrap();
        let local_endpoint = endpoint.clone();
        let s3client = S3Client::new(provider, local_endpoint);


        loop {
            let mut operation = Operation::default();
            object = format!("{}{:04}", base_object_name, count+1);

            // Synthetic buffer creation to simulate an on disk object
            let mut buffer: Vec<u8>;
            zero_fill_buffer!(buffer, size);

            let mut request = PutObjectRequest::default();
            request.bucket = bucket.to_string();
            request.key = object.clone();
            request.body = Some(&buffer);

            match s3client.put_object(&request, Some(&mut operation)) {
                Ok(output) => {},
                Err(e) => {
                    println_color_red!("Failed to put [{}/{}] - {}", bucket, object, e);
                },
            }

            if now.elapsed() >= duration {
                operations.push(operation);
                break;
            }

            operations.push(operation);
            count += 1;
        }
    }

    Ok(())
}

fn put_bench<'a, 'b, P, D>(bucket: &str,
                           path: &str,
                           base_object_name: &str,
                           duration: &'b Duration,
                           iterations: u64,
                           len: u64,
                           operations: &'a mut Vec<Operation>,
                           client: &Client<P, D>) -> Result<(), S3Error>
                           where P: AwsCredentialsProvider,
                                 D: DispatchSignedRequest,
{
    let mut key: String;
    let mut object: String;

    if iterations > 0 {
        for i in 0..iterations {
            let mut operation: Operation;
            operation = Operation::default();
            key = format!("{}{:04}", base_object_name, i+1);
            object = format!("{}/{}", path, key);
            let result = put_object(bucket, &key, &object, len, Some(&mut operation), client);
            operations.push(operation);
        }
    } else if duration.as_secs() > 0 {
        let mut count: u64 = 0;
        let now = Instant::now();

        loop {
            let mut operation: Operation;
            operation = Operation::default();
            key = format!("{}{:04}", base_object_name, count+1);
            object = format!("{}/{}", path, key);
            let result = put_object(bucket, &key, &object, len, Some(&mut operation), client);
            operations.push(operation);
            if now.elapsed() >= *duration {
                break;
            }
            count += 1;
        }
    }

    Ok(())
}

// Limited in file size.
fn get_object<P, D>(bucket: &str,
                    object: &str,
                    operation: Option<&mut Operation>,
                    client: &Client<P, D>) -> Result<(), S3Error>
                    where P: AwsCredentialsProvider,
                          D: DispatchSignedRequest,
{
    let mut request = GetObjectRequest::default();
    request.bucket = bucket.to_string();
    request.key = object.to_string();

    object_get(&request, operation, client)
}

// Common portion of get_object... functions
fn object_get<P, D>(request: &GetObjectRequest,
                    operation: Option<&mut Operation>,
                    client: &Client<P, D>) -> Result<(), S3Error>
                    where P: AwsCredentialsProvider,
                          D: DispatchSignedRequest,
{
    match client.s3client.get_object(&request, operation) {
        Ok(output) => {
            Ok(())
        },
        Err(e) => {
            let error = format!("{:#?}", e);
            println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
            Err(S3Error::new(error))
        },
    }
}

fn get_object_range<P, D>(bucket: &str,
                          object: &str,
                          offset: u64,
                          len: u64,
                          operation: Option<&mut Operation>,
                          client: &Client<P, D>)
                          -> Result<(), S3Error>
                          where P: AwsCredentialsProvider,
                                D: DispatchSignedRequest,
{
    let mut request = GetObjectRequest::default();
    request.bucket = bucket.to_string();
    request.key = object.to_string();
    request.range = Some(format!("bytes={}-{}", offset, len));

    object_get(&request, operation, client)
}

// Limited in file size. Max is 5GB but should use Multipart upload for larger than 15MB.
fn put_object<P, D>(bucket: &str,
                    key: &str,
                    object: &str,
                    len: u64,
                    operation: Option<&mut Operation>,
                    client: &Client<P, D>) -> Result<(), S3Error>
                    where P: AwsCredentialsProvider,
                          D: DispatchSignedRequest,
{
    let mut buffer: Vec<u8>;
    if len == 0 {
        let file = File::open(object).unwrap();
        let metadata = file.metadata().unwrap();

        buffer = Vec::with_capacity(metadata.len() as usize);

        match file.take(metadata.len()).read_to_end(&mut buffer) {
            Ok(_) => {},
            Err(e) => {
                let error = format!("Error reading file {}", e);
                return Err(S3Error::new(error));
            },
        }
    } else {
        zero_fill_buffer!(buffer, len);
    }

    let correct_key = if key.is_empty() {
        let path = Path::new(object);
        path.file_name().unwrap().to_str().unwrap().to_string()
    } else {
        key.to_string()
    };

    let mut request = PutObjectRequest::default();
    request.bucket = bucket.to_string();
    request.key = correct_key;
    request.body = Some(&buffer);

    match client.s3client.put_object(&request, operation) {
        Ok(output) => {
            match client.output.format {
                OutputFormat::Serialize => {
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
                },
                OutputFormat::Plain => {
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
                },
                OutputFormat::JSON => {
                    println_color_quiet!(client.is_quiet,
                                         client.output.color,
                                         "{}",
                                         json::encode(&output).unwrap_or("{}".to_string()));
                },
                OutputFormat::PrettyJSON => {
                    println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output));
                },
                OutputFormat::Simple => {
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
                },
                _ => {},
            }
            Ok(())
        },
        Err(e) => {
            let error = format!("{:#?}", e);
            println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
            Err(S3Error::new(error))
        },
    }
}

fn abort_multipart_upload<P, D>(bucket: &str,
                                object: &str,
                                id: &str,
                                client: &Client<P, D>) -> Result<(), S3Error>
                                where P: AwsCredentialsProvider,
                                      D: DispatchSignedRequest,
{
    let mut request = MultipartUploadAbortRequest::default();
    request.bucket = bucket.to_string();
    request.upload_id = id.to_string();
    request.key = object.to_string();

    match client.s3client.multipart_upload_abort(&request) {
        Ok(output) => {
            match client.output.format {
                OutputFormat::Serialize => {
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
                },
                OutputFormat::Plain => {
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
                },
                OutputFormat::JSON => {
                    println_color_quiet!(client.is_quiet,
                                         client.output.color,
                                         "{}",
                                         json::encode(&output).unwrap_or("{}".to_string()));
                },
                OutputFormat::PrettyJSON => {
                    println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output));
                },
                OutputFormat::Simple => {
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
                },
                _ => {},
            }
        },
        Err(e) => {
            let error = format!("{:#?}", e);
            println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
            return Err(S3Error::new(error));
        },
    }

    Ok(())
}

/// Important - Do not leave incomplete uploads. You will be charged for those parts that have not
/// been completed. You runs ```s3lsio ls s3://<bucket name> multi``` to find out the uploads
/// that have not completed and then you an run ```s3lsio abort <upload_id> s3://<bucket_name>/<object_name>```
/// to abort the upload process.
///
/// You can also apply a bucket policy to automatically abort any uploads that have not completed
/// after so many days.
fn put_multipart_upload<P, D>(bucket: &str,
                              key: &str,
                              object: &str,
                              part_size: u64,
                              compute_hash: bool,
                              client: &Client<P, D>)
                              -> Result<(), S3Error>
                              where P: AwsCredentialsProvider,
                                    D: DispatchSignedRequest,
{
    let correct_key = if key.is_empty() {
        let path = Path::new(object);
        path.file_name().unwrap().to_str().unwrap().to_string()
    } else {
        key.to_string()
    };

    // Create multipart
    let create_multipart_upload: MultipartUploadCreateOutput;
    let mut request = MultipartUploadCreateRequest::default();
    request.bucket = bucket.to_string();
    request.key = correct_key.clone();

    match client.s3client.multipart_upload_create(&request) {
        Ok(output) => {
            create_multipart_upload = output;
        },
        Err(e) => {
            let error = format!("Multipart-Upload: {:#?}", e);
            return Err(S3Error::new(error));
        },
    }

    let upload_id: &str = &create_multipart_upload.upload_id;
    let mut parts_list: Vec<String> = Vec::new();

    // NB: To begin with the multipart will be a sequential upload in this thread! Aftwards, it will
    // be split out to a multiple of threads...

    let file = File::open(object).unwrap();
    let metadata = file.metadata().unwrap();

    let mut part_buffer: Vec<u8> = Vec::with_capacity(metadata.len() as usize);

    match file.take(metadata.len()).read_to_end(&mut part_buffer) {
        Ok(_) => {},
        Err(e) => {
            let error = format!("Multipart-Upload: Error reading file {}", e);
            return Err(S3Error::new(error));
        },
    }

    let mut request = MultipartUploadPartRequest::default();
    request.bucket = bucket.to_string();
    request.upload_id = upload_id.to_string();
    request.key = correct_key.clone();

    request.body = Some(&part_buffer);
    request.part_number = 1;
    // Compute hash - Hash is slow

    if compute_hash {
        let hash = md5::compute(request.body.unwrap()).to_base64(STANDARD);
        request.content_md5 = Some(hash);
    }

    match client.s3client.multipart_upload_part(&request) {
        Ok(output) => {
            // Collecting the partid in a list.
            let new_output = output.clone();
            parts_list.push(output);

            match client.output.format {
                OutputFormat::Serialize => {
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", new_output);
                },
                OutputFormat::Plain => {
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", new_output);
                },
                OutputFormat::JSON => {
                    println_color_quiet!(client.is_quiet,
                                         client.output.color,
                                         "{}",
                                         json::encode(&new_output).unwrap_or("{}".to_string()));
                },
                OutputFormat::PrettyJSON => {
                    println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&new_output));
                },
                OutputFormat::Simple => {
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", new_output);
                },
                _ => {},
            }
        },
        Err(e) => {
            let error = format!("Multipart-Upload Part: {:#?}", e);
            println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
            return Err(S3Error::new(error));
        },
    }
    // End of upload

    // Complete multipart
    let item_list: Vec<u8>;

    let mut request = MultipartUploadCompleteRequest::default();
    request.bucket = bucket.to_string();
    request.upload_id = upload_id.to_string();
    request.key = correct_key;

    // parts_list gets converted to XML and sets the item_list.
    match multipart_upload_finish_xml(&parts_list) {
        Ok(parts_in_xml) => item_list = parts_in_xml,
        Err(e) => {
            let error = format!("Multipart-Upload XML: {:#?}", e);
            println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
            return Err(S3Error::new(error));
        },
    }

    request.multipart_upload = Some(&item_list);

    match client.s3client.multipart_upload_complete(&request) {
        Ok(output) => {
            match client.output.format {
                OutputFormat::Serialize => {
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
                },
                OutputFormat::Plain => {
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
                },
                OutputFormat::JSON => {
                    println_color_quiet!(client.is_quiet,
                                         client.output.color,
                                         "{}",
                                         json::encode(&output).unwrap_or("{}".to_string()));
                },
                OutputFormat::PrettyJSON => {
                    println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output));
                },
                OutputFormat::Simple => {
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
                },
                _ => {},
            }
        },
        Err(e) => {
            let error = format!("Multipart-Upload Complete: {:#?}", e);
            println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
            return Err(S3Error::new(error));
        },
    }

    Ok(())
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

    //let mut pbb = ProgressBar::new(100);

    for i in 0..virtual_users {
        let t_arc = arc.clone();
        let t_arc_start_times = arc_start_times.clone();
        let t_arc_end_times = arc_end_times.clone();
        let t_bucket = bucket.clone();
        let t_endpoint = endpoint.clone();

        //pbb.inc();

        // Spawn the threads which represent virtual users
        let handle = thread::spawn(move || {
            let mut operations: Vec<Operation> = Vec::new();
            let thread_name = format!("thread_{:04}", i+1);
            let base_object_name = format!("{}/file{:04}", thread_name, i+1);
            // NB: Each thread has it's own virtual directory in S3 for the given bucket.

            match method {
                Commands::get => {
                    let result = do_get_bench(&t_bucket, &base_object_name, duration2, iterations, None, t_endpoint, &mut operations);
                },
                Commands::put => {
                    let result = do_put_bench(&t_bucket, &base_object_name, duration2, iterations, size, t_endpoint, &mut operations);
                },
                Commands::range => {
                    let range = format!("bytes={}-{}", offset, len);
                    let result = do_get_bench(&t_bucket, &base_object_name, duration2, iterations, Some(&range), t_endpoint, &mut operations);
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

    //pbb.finish();

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
