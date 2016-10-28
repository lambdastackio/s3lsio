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
use std::fs::File;
use std::ffi::OsStr;
use std::thread;
use std::time::{Duration, Instant};

use md5;
use term;
use rustc_serialize::json;
use rustc_serialize::base64::{STANDARD, ToBase64};

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

use Client;
use Output;
use OutputFormat;
use Commands;

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

fn get_range_bench<'a, 'b, P, D>(bucket: &str,
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
                        operations: &'a mut Vec<Operation>) -> Result<(), S3Error>
{
    let mut object: String;

    if iterations > 0 {
        for i in 0..iterations {
            let mut operation = Operation::default();

            // NB: For benchmarking, the objects are synthetic and in a predictable naming format.
            object = format!("{}{:04}", base_object_name, i+1);

            let mut request = GetObjectRequest::default();
            request.bucket = bucket.to_string();
            request.key = object;

            let provider = DefaultCredentialsProviderSync::new(None).unwrap();
            let endpoint = Endpoint::new(Region::UsEast1,
                                         Signature::V2,
                                         None,
                                         None,
                                         Some(format!("s3lsio - {}", "V2")));

            let s3client = S3Client::new(provider, endpoint);
            match s3client.get_object(&request, Some(&mut operation)) {
                Ok(output) => {},
                Err(e) => {
                    println!("Failed to get it...");
                    //let error = format!("{:#?}", e);
                    //println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
                    //Err(S3Error::new(error))
                },
            }

            operations.push(operation);
        }
    } else if duration.as_secs() > 0 {
        let mut count: u64 = 0;
        let now = Instant::now();

        loop {
            let mut operation = Operation::default();
            object = format!("{}{:04}", base_object_name, count+1);
            //let result = get_object(bucket, &object, Some(&mut operation), client);
            operations.push(operation);
            if now.elapsed() >= duration {
                break;
            }
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

    let correct_key: String;
    if key.is_empty() {
        let path = Path::new(object);
        correct_key = path.file_name().unwrap().to_str().unwrap().to_string();
    } else {
        correct_key = key.to_string();
    }
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
