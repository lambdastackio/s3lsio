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
use md5;

use term;
use rustc_serialize::json;
use rustc_serialize::base64::{STANDARD, ToBase64};

use clap::ArgMatches;
use aws_sdk_rust::aws::errors::s3::S3Error;
use aws_sdk_rust::aws::common::credentials::AwsCredentialsProvider;
use aws_sdk_rust::aws::common::request::DispatchSignedRequest;
use aws_sdk_rust::aws::common::common::Operation;
use aws_sdk_rust::aws::common::params::*;
use aws_sdk_rust::aws::s3::acl::*;
use aws_sdk_rust::aws::s3::bucket::*;
use aws_sdk_rust::aws::s3::object::*;
use aws_sdk_rust::aws::s3::admin::*;

// Use this for signing the admin feature for Ceph RGW
use aws_sdk_rust::aws::common::signature::*;

use Client;
use Output;
use OutputFormat;
use Commands;

// 5MB minimum size for multipart_uploads. Only last part can be less.
const PART_SIZE_MIN: u64 = 5242880;

/// Commands
pub fn commands<P, D>(matches: &ArgMatches, cmd: Commands, client: &mut Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut bucket: &str = "";
    let mut object: String = "".to_string();
    let mut last: &str = "";
    // Make sure the s3 schema prefix is present
    //let (scheme, tmp_bucket) = matches.value_of("bucket").unwrap_or("s3:// ").split_at(5);
    match matches.value_of("bucket") {
        Some(buck) => {
            let mut scheme: &str = "";
            let mut tmp_bucket: &str = "";

            if buck.contains("s3://") {
                scheme = "s3://";
                tmp_bucket = &buck[5..];
            } else {
                tmp_bucket = buck.clone();
            }
            if tmp_bucket.contains('/') {
                let components: Vec<&str> = tmp_bucket.split('/').collect();
                let mut first: bool = true;
                let mut object_first: bool = true;

                for part in components {
                    if first {
                        bucket = part;
                    } else {
                        if !object_first {
                            object += "/";
                        }
                        object_first = false;
                        object += part;
                        last = part;
                    }
                    first = false;
                }
            } else {
                match tmp_bucket.trim() {
                    "." | "*" | "$" | "s3://" => {},
                    a @ _ => { bucket = a; },
                }
                object = "".to_string();
            }
        },
        None => {},
    }

    match cmd {
        Commands::get => {
            let mut path = matches.value_of("path").unwrap_or("").to_string();
            if path.is_empty() {
                path = last.to_string();
            } else {
                path = format!("{}{}{}",
                               path,
                               if path.ends_with('/') {
                                   ""
                               } else {
                                   "/"
                               },
                               last);
            }

            cmd_get(bucket, &object, &path, client);
            Ok(())
        },
        Commands::put => {
            let path = matches.value_of("path").unwrap_or("");
            let part_size: u64 = matches.value_of("size").unwrap_or("0").parse().unwrap_or(0);
            cmd_put(bucket, &object, path, part_size, client);
            Ok(())
        },
        Commands::cp => {
            let mut get: bool = true;
            let mut path = matches.value_of("path").unwrap_or("").to_string();
            if path.contains("s3://") {
                get = false;
                let (scheme, tmp_bucket) = matches.value_of("path").unwrap_or("s3:// ").split_at(5);

                if tmp_bucket.contains('/') {
                    let components: Vec<&str> = tmp_bucket.split('/').collect();
                    let mut first: bool = true;
                    let mut object_first: bool = true;

                    for part in components {
                        if first {
                            bucket = part;
                        } else {
                            if !object_first {
                                object += "/";
                            }
                            object_first = false;
                            object += part;
                            //last = part;
                        }
                        first = false;
                    }
                } else {
                    bucket = tmp_bucket.trim();
                    object = "".to_string();
                }
                path = matches.value_of("bucket").unwrap_or("").to_string();
            }

            if get {
                cmd_get(bucket, &object, &path, client);
            } else {
                let part_size: u64 = matches.value_of("size").unwrap_or("0").parse().unwrap_or(0);
                cmd_put(bucket, &object, &path, part_size, client);
            }

            Ok(())
        },
        Commands::range => {
            let offset: u64 = matches.value_of("offset").unwrap_or("0").parse().unwrap_or(0);
            let len: u64 = matches.value_of("len").unwrap_or("0").parse().unwrap_or(0);
            let mut path = matches.value_of("path").unwrap_or("").to_string();
            if path.is_empty() {
                path = last.to_string();
            }
            if len == 0 {
                println_color_quiet!(client.is_quiet, client.error.color, "Error Byte-Range request: Len must be > 0");
                return Err(S3Error::new("Error Byte-Range request: Len must be > 0"));
            }
            let mut operation = Operation::default();
            let result = get_object_range(bucket, &object, offset, len, &path, Some(&mut operation), client);
            //println!("{:#?}", operation);
            Ok(())
        },
        Commands::rm => {
            let version = matches.value_of("version").unwrap_or("");
            let mut operation = Operation::default();
            let result = delete_object(bucket, &object, version, Some(&mut operation), client);
            //println!("{:#?}", operation);
            Ok(())
        },
        Commands::abort => {
            let upload_id = matches.value_of("upload_id").unwrap_or("");
            let result = abort_multipart_upload(bucket, &object, upload_id, client);
            Ok(())
        },
        Commands::acl => {
            if object.is_empty() {
                let acl = try!(get_bucket_acl(bucket, client));
                match client.output.format {
                    OutputFormat::Plain => {
                        // Could have already been serialized before being passed to this function.
                        println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", acl);
                    },
                    OutputFormat::JSON => {
                        println_color_quiet!(client.is_quiet,
                                             client.output.color,
                                             "{}",
                                             json::encode(&acl).unwrap_or("{}".to_string()));
                    },
                    OutputFormat::PrettyJSON => {
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&acl));
                    },
                    OutputFormat::None | OutputFormat::NoneAll => {},
                    e => println_color_quiet!(client.is_quiet, client.error.color, "Error: Format - {:#?}", e),
                }
            } else {
                let acl = try!(get_object_acl(bucket, &object, client));
            }
            Ok(())
        },
        Commands::head => {
            if object.is_empty() {
                let list = try!(get_bucket_head(bucket, client));
            } else {
                let list = try!(get_object_head(bucket, &object, client));
            }
            Ok(())
        },
        Commands::ls => {
            // NB: object is prefix for ls cmd
            let option = matches.value_of("option").unwrap_or("");
            if bucket.is_empty() {
                let list = try!(get_buckets_list(client));
            } else if bucket.contains('/') {
                let components: Vec<&str> = bucket.split('/').collect();
                bucket = components[0];
                if components[1].is_empty() {
                    // List objects in bucket.
                    if option.is_empty() {
                        let list = try!(get_object_list(bucket, &object, 1, client));
                    } else if option == "multi" {
                        let upload_id = matches.value_of("upload_id").unwrap_or("");
                        let list = try!(get_object_multipart_list(bucket, upload_id, &object, client));
                    } else {
                        let list = try!(get_object_version_list(bucket, &object, option, client));
                    }
                }
            } else if option.is_empty() {
                    let list = try!(get_object_list(bucket, &object, 1, client));
                } else if option == "multi" {
                    let upload_id = matches.value_of("upload_id").unwrap_or("");
                    let list = try!(get_object_multipart_list(bucket, upload_id, &object, client));
                } else {
                    let list = try!(get_object_version_list(bucket, &object, option, client));
                }

            Ok(())
        },
        /// create new bucket
        Commands::mb => {
            if bucket.is_empty() {
                println_color_quiet!(client.is_quiet, term::color::RED, "missing bucket name");
                Err(S3Error::new("missing bucket name"))
            } else {
                let result = create_bucket(bucket, client);
                Ok(())
            }
        },
        Commands::rb => {
            let result = delete_bucket(bucket, client);
            Ok(())
        },
        Commands::setacl => {
            let result = try!(set_bucket_acl(matches, bucket, client));
            Ok(())
        },
        Commands::setver => {
            let list = try!(set_bucket_versioning(matches, bucket, client));
            Ok(())
        },
        Commands::ver => {
            let list = try!(get_bucket_versioning(bucket, client));
            Ok(())
        },
        // Ceph RGW Admin Section...
        Commands::bucket => {
            let list = try!(buckets(matches, bucket, client));
            Ok(())
        },
        Commands::cap => {
            let list = try!(caps(matches, bucket, client));
            Ok(())
        },
        Commands::keys => {
            let list = try!(keys(matches, bucket, client));
            Ok(())
        },
        Commands::object => {
            let list = try!(objects(matches, bucket, &object, client));
            Ok(())
        },
        Commands::quota => {
            let list = try!(quota(matches, bucket, client));
            Ok(())
        },
        Commands::user => {
            let list = try!(user(matches, bucket, client));
            Ok(())
        },
        Commands::usage => {
            let list = try!(usage(matches, bucket, client));
            Ok(())
        },
        _ => {
            Ok(())
        }
    };

    Ok(())
}

fn cmd_get<P, D>(bucket: &str, object: &str, path: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    if client.is_time {
        let mut operation: Operation;
        operation = Operation::default();
        let result = get_object(bucket, &object, &path, Some(&mut operation), client);
        match client.output.format {
            OutputFormat::Serialize => {
                // Could have already been serialized before being passed to this function.
                println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", operation);
            },
            OutputFormat::Plain => {
                // Could have already been serialized before being passed to this function.
                println_color_quiet!(client.is_quiet, client.output.color, "{:?}", operation);
            },
            OutputFormat::JSON => {
                println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", operation);
            },
            OutputFormat::PrettyJSON => {
                println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", operation);
            },
            OutputFormat::Simple => {
                // Could have already been serialized before being passed to this function.
                println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", operation);
            },
            _ => {},
        }
    } else {
        let result = get_object(bucket, &object, &path, None, client);
    }

    Ok(())
}
fn cmd_put<P, D>(bucket: &str, object: &str, path: &str, part_size: u64, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    if client.is_time {
        let mut operation: Operation;
        operation = Operation::default();
        if part_size < PART_SIZE_MIN {
            let result = put_object(bucket, &object, path, Some(&mut operation), client);
            match client.output.format {
                OutputFormat::Serialize => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", operation);
                },
                OutputFormat::Plain => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{:?}", operation);
                },
                OutputFormat::JSON => {
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", operation);
                },
                OutputFormat::PrettyJSON => {
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", operation);
                },
                OutputFormat::Simple => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", operation);
                },
                _ => {},
            }
        } else {
            let compute_hash: bool = false; // Change this to an option via the config
            let result = put_multipart_upload(bucket, &object, path, part_size, compute_hash, client);
        }
    } else if part_size < PART_SIZE_MIN {
        let result = put_object(bucket, &object, path, None, client);
    } else {
        let compute_hash: bool = false; // Change this to an option via the config
        let result = put_multipart_upload(bucket, &object, path, part_size, compute_hash, client);
    }

    Ok(())
}

fn create_bucket<P, D>(bucket: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut request = CreateBucketRequest::default();
    request.bucket = bucket.to_string();

    match client.s3client.create_bucket(&request) {
        Ok(_) => {
            if (client.output.format != OutputFormat::None) || (client.output.format != OutputFormat::NoneAll) {
                println_color_quiet!(client.is_quiet, client.output.color, "Success");
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

fn delete_bucket<P, D>(bucket: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let request = DeleteBucketRequest { bucket: bucket.to_string() };

    match client.s3client.delete_bucket(&request) {
        Ok(_) => {
            if (client.output.format != OutputFormat::None) || (client.output.format != OutputFormat::NoneAll) {
                println_color_quiet!(client.is_quiet, client.output.color, "Success");
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

// Get functions...
fn get_bucket_head<P, D>(bucket: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let request = HeadBucketRequest { bucket: bucket.to_string() };

    match client.s3client.head_bucket(&request) {
        Ok(_) => {
            if (client.output.format != OutputFormat::None) || (client.output.format != OutputFormat::NoneAll) {
                // May want to put in json format later??
                println_color_quiet!(client.is_quiet, client.output.color, "Bucket exists");
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

fn get_bucket_versioning<P, D>(bucket: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let request = GetBucketVersioningRequest { bucket: bucket.to_string() };

    match client.s3client.get_bucket_versioning(&request) {
        Ok(output) => {
            match client.output.format {
                OutputFormat::Serialize => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
                },
                OutputFormat::Plain => {
                    // Could have already been serialized before being passed to this function.
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
                    println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output));
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

fn get_bucket_acl<P, D>(bucket: &str, client: &Client<P, D>) -> Result<AccessControlPolicy, S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut request = GetBucketAclRequest::default();
    request.bucket = bucket.to_string();

    match client.s3client.get_bucket_acl(&request) {
        Ok(acl) => Ok(acl),
        Err(e) => {
            let error = format!("{:#?}", e);
            println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
            Err(S3Error::new(error))
        },
    }
}

fn get_buckets_list<P, D>(client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    match client.s3client.list_buckets() {
        Ok(output) => {
            match client.output.format {
                OutputFormat::Serialize => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
                },
                OutputFormat::Plain => {
                    // Could have already been serialized before being passed to this function.
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
                    for bucket in output.buckets {
                        println_color_quiet!(client.is_quiet, client.output.color, "s3://{}/", bucket.name);
                    }
                },
                _ => {},
            }
            Ok(())
        },
        Err(e) => {
            let format = format!("{:#?}", e);
            let error = S3Error::new(format);
            println_color_quiet!(client.is_quiet, client.error.color, "{:?}", error);
            Err(error)
        },
    }
}

// Set functions...
fn set_bucket_acl<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{

    let acl: CannedAcl;
    let cli_acl = matches.value_of("acl").unwrap_or("").to_string().to_lowercase();

    match cli_acl.as_ref() {
        "public-read" => acl = CannedAcl::PublicRead,
        "public-rw" | "public-readwrite" => acl = CannedAcl::PublicReadWrite,
        "private" => acl = CannedAcl::Private,
        _ => {
            println_color_quiet!(client.is_quiet,
                                 client.error.color,
                                 "missing acl: public-read, public-rw, public-readwrite or private");
            return Err(S3Error::new("missing acl: public-read, public-rw, public-readwrite or private"));
        },
    }

    let mut request = PutBucketAclRequest::default();
    request.bucket = bucket.to_string();

    // get acl option...
    request.acl = Some(acl);

    match client.s3client.put_bucket_acl(&request) {
        Ok(output) => {
            let acl = get_bucket_acl(bucket, client);
            if let Ok(acl) = acl {
                match client.output.format {
                    OutputFormat::Serialize => {
                        // Could have already been serialized before being passed to this function.
                        println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", acl);
                    },
                    OutputFormat::Plain => {
                        // Could have already been serialized before being passed to this function.
                        println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", acl);
                    },
                    OutputFormat::JSON => {
                        println_color_quiet!(client.is_quiet,
                                             client.output.color,
                                             "{}",
                                             json::encode(&acl).unwrap_or("{}".to_string()));
                    },
                    OutputFormat::PrettyJSON => {
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&acl));
                    },
                    OutputFormat::Simple => {
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&acl));
                    },
                    _ => {},
                }
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

fn set_bucket_versioning<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let cli_ver = matches.value_of("ver").unwrap_or("").to_string().to_lowercase();

    let request = PutBucketVersioningRequest {
        bucket: bucket.to_string(),
        versioning_configuration: VersioningConfiguration {
            status: if cli_ver == "on" {
                "Enabled".to_string()
            } else {
                "Suspended".to_string()
            },
            mfa_delete: "".to_string(),
        },
        mfa: None,
        content_md5: None,
    };

    match client.s3client.put_bucket_versioning(&request) {
        Ok(()) => {
            if (client.output.format != OutputFormat::None) || (client.output.format != OutputFormat::NoneAll) {
                println_color_quiet!(client.is_quiet, client.output.color, "Success");
            }
            Ok(())
        },
        Err(e) => {
            println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
            Err(e)
        },
    }
}

// Objects...
fn get_object_list<P, D>(bucket: &str, prefix: &str, list_version: u16, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut request = ListObjectsRequest::default();
    request.bucket = bucket.to_string();
    if !prefix.is_empty() {
        request.prefix = Some(prefix.to_string());
    }
    if list_version == 2 {
        request.version = Some(2);
    } // default to original version of list bucket

    match client.s3client.list_objects(&request) {
        Ok(output) => {
            match client.output.format {
                OutputFormat::Serialize => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
                },
                OutputFormat::Plain => {
                    // Could have already been serialized before being passed to this function.
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
                    for object in output.contents {
                        println_color_quiet!(client.is_quiet, client.output.color, "s3://{}/{}", bucket, object.key);
                    }
                },
                _ => {},
            }

            Ok(())
        },
        Err(error) => {
            let format = format!("{:#?}", error);
            let error = S3Error::new(format);
            println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", error);
            Err(error)
        },
    }
}

fn get_object_version_list<P, D>(bucket: &str,
                                 prefix: &str,
                                 version: &str,
                                 client: &Client<P,D>)
                                 -> Result<(), S3Error>
                                 where P: AwsCredentialsProvider,
                                       D: DispatchSignedRequest {
    let mut request = ListObjectVersionsRequest::default();
    request.bucket = bucket.to_string();
    if !prefix.is_empty() {
        request.prefix = Some(prefix.to_string());
    }

    match client.s3client.list_object_versions(&request) {
        Ok(output) => {
            match client.output.format {
                OutputFormat::Serialize => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
                },
                OutputFormat::Plain => {
                    // Could have already been serialized before being passed to this function.
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
                _ => {},
            }

            Ok(())
        },
        Err(error) => {
            let format = format!("{:#?}", error);
            let error = S3Error::new(format);
            println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", error);
            Err(error)
        },
    }
}

fn get_object_multipart_list<P, D>(bucket: &str, upload_id: &str, key: &str, client: &Client<P, D>)
                                   -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    if upload_id.is_empty() {
        let mut request = MultipartUploadListRequest::default();
        request.bucket = bucket.to_string();

        match client.s3client.multipart_upload_list(&request) {
            Ok(output) => {
                match client.output.format {
                    OutputFormat::Serialize => {
                        // Could have already been serialized before being passed to this function.
                        println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
                    },
                    OutputFormat::Plain => {
                        // Could have already been serialized before being passed to this function.
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
                    _ => {},
                }

                Ok(())
            },
            Err(error) => {
                let format = format!("{:#?}", error);
                let error = S3Error::new(format);
                println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", error);
                Err(error)
            },
        }
    } else {
        let mut request = MultipartUploadListPartsRequest::default();
        request.bucket = bucket.to_string();
        request.upload_id = upload_id.to_string();
        request.key = key.to_string();

        match client.s3client.multipart_upload_list_parts(&request) {
            Ok(output) => {
                match client.output.format {
                    OutputFormat::Serialize => {
                        // Could have already been serialized before being passed to this function.
                        println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
                    },
                    OutputFormat::Plain => {
                        // Could have already been serialized before being passed to this function.
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
}

// Limited in file size.
fn get_object<P, D>(bucket: &str, object: &str, path: &str, operation: Option<&mut Operation>, client: &Client<P, D>)
                    -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut request = GetObjectRequest::default();
    request.bucket = bucket.to_string();
    request.key = object.to_string();

    object_get(&request, path, operation, client)
}

// Common portion of get_object... functions
fn object_get<P, D>(request: &GetObjectRequest, path: &str, operation: Option<&mut Operation>, client: &Client<P, D>)
                    -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    match client.s3client.get_object(&request, operation) {
        Ok(output) => {
            // NoneAll means no writing to disk or stdout
            if client.output.format != OutputFormat::NoneAll {
                let mut file = File::create(path).unwrap();
                match file.write_all(output.get_body()) {
                    Ok(_) => {
                        // NOTE: Need to remove body from output (after it writes out) by making it mut so that
                        // items below can output metadata OR place body in different element than others.
                        match client.output.format {
                            // NOTE: Operation use of SteadyTime can be (by default) be serialized w/o effort :(
                            OutputFormat::Serialize => {
                                println_color_quiet!(client.is_quiet, client.output.color, "Success");
                            },
                            OutputFormat::Plain => {
                                println_color_quiet!(client.is_quiet, client.output.color, "Success");
                            },
                            OutputFormat::JSON => {
                                println_color_quiet!(client.is_quiet, client.output.color, "Success");
                            },
                            OutputFormat::PrettyJSON => {
                                println_color_quiet!(client.is_quiet, client.output.color, "Success");
                            },
                            OutputFormat::Simple => {
                                println_color_quiet!(client.is_quiet, client.output.color, "Success");
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
            } else {
                Ok(())
            }
        },
        Err(e) => {
            let error = format!("{:#?}", e);
            println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
            Err(S3Error::new(error))
        },
    }
}

fn get_object_range<P, D>(bucket: &str, object: &str, offset: u64, len: u64, path: &str,
                          operation: Option<&mut Operation>, client: &Client<P, D>)
                          -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut request = GetObjectRequest::default();
    request.bucket = bucket.to_string();
    request.key = object.to_string();
    request.range = Some(format!("bytes={}-{}", offset, len));

    object_get(&request, path, operation, client)
}

fn get_object_head<P, D>(bucket: &str, object: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut request = HeadObjectRequest::default();
    request.bucket = bucket.to_string();
    request.key = object.to_string();

    match client.s3client.head_object(&request) {
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

fn get_object_acl<P, D>(bucket: &str, object: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut request = GetObjectAclRequest::default();
    request.bucket = bucket.to_string();
    request.key = object.to_string();

    match client.s3client.get_object_acl(&request) {
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

// Limited in file size. Max is 5GB but should use Multipart upload for larger than 15MB.
fn put_object<P, D>(bucket: &str, key: &str, object: &str, operation: Option<&mut Operation>, client: &Client<P, D>)
                    -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let file = File::open(object).unwrap();
    let metadata = file.metadata().unwrap();

    let mut buffer: Vec<u8> = Vec::with_capacity(metadata.len() as usize);

    match file.take(metadata.len()).read_to_end(&mut buffer) {
        Ok(_) => {},
        Err(e) => {
            let error = format!("Error reading file {}", e);
            return Err(S3Error::new(error));
        },
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

fn abort_multipart_upload<P, D>(bucket: &str, object: &str, id: &str, client: &Client<P, D>) -> Result<(), S3Error>
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
fn put_multipart_upload<P, D>(bucket: &str, key: &str, object: &str, part_size: u64, compute_hash: bool,
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

fn delete_object<P, D>(bucket: &str, object: &str, version: &str, operation: Option<&mut Operation>,
                       client: &Client<P, D>)
                       -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut request = DeleteObjectRequest::default();
    request.bucket = bucket.to_string();
    request.key = object.to_string();
    if !version.is_empty() {
        request.version_id = Some(version.to_string());
    }

    match client.s3client.delete_object(&request, operation) {
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

// CEPH RGW ONLY SECTION
// Check for is_admin. If true then it's assumed this is for Ceph's RGW and NOT AWS.
fn buckets<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let is_admin = client.is_admin;

    if is_admin {
        let mut method = String::from("GET");

        let mut command = matches.value_of("command").unwrap_or("");
        let mut user = matches.value_of("user").unwrap_or("").to_string();
        let stats = matches.value_of("stats").unwrap_or("false").to_string().to_lowercase();

        // Make into macro...
        match user.clone().trim() {
            "" | "." | "*" | "$" | "s3://" => { user = "".to_string(); },
            a @ _ => { user = a.to_string(); },
        }
        match command.clone().trim() {
            "" | "." | "*" | "$" | "s3://" => { command = ""; },
            a @ _ => { command = a; },
        }

        let mut path: String = "admin/".to_string();
        let mut params = Params::new();
        let mut error: String = "".to_string();
        let mut path_options: Option<String> = None;

        match command {
            "delete" => {
                if bucket.is_empty() {
                    error += &format!("Bucket value must be valid for delete command. ");
                }
                if !error.is_empty() {
                    let e = S3Error::new(error);
                    println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
                    return Err(e);
                }
                path += "bucket";
                method = "DELETE".to_string();

                params.put("bucket", bucket);
                // NB: Could add an additional option but for now just remove the objects first.
                params.put("purge-objects", "true");
            },
            "index" => {
                if bucket.is_empty() {
                    error += &format!("Bucket value must be valid for index command. ");
                }
                if !error.is_empty() {
                    let e = S3Error::new(error);
                    println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
                    return Err(e);
                }
                let fix = matches.value_of("fix").unwrap_or("false").to_string().to_lowercase();
                let check = matches.value_of("check").unwrap_or("false").to_string().to_lowercase();

                path += "bucket";
                path_options = Some("?index&".to_string());
                params.put("bucket", bucket);
                if !fix.is_empty() {
                    params.put("fix", &fix);
                }
                if !check.is_empty() {
                    params.put("check-objects", &check);
                }
            },
            "link" => {
                if bucket.is_empty() {
                    error += &format!("Bucket value must be valid for link command. ");
                }
                if user.is_empty() {
                    error += &format!("User value must be valid for link command. ");
                }
                if !error.is_empty() {
                    let e = S3Error::new(error);
                    println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
                    return Err(e);
                }
                method = "PUT".to_string();

                path += "bucket";
                params.put("bucket", bucket);
                params.put("uid", &user);
            },
            "ls" => {
                path += "metadata/bucket";
                if !user.is_empty() {
                    params.put("uid", &user);
                }
            },
            "policy" => {
                path += "bucket";
                path_options = Some("?policy&".to_string());
                if !bucket.is_empty() {
                    params.put("bucket", bucket);
                } else {
                    let e = S3Error::new("Bucket must be valid");
                    println_color_quiet!(client.is_quiet, client.error.color, "{}", e);
                    return Err(e);
                }
            },
            "stats" => {
                path += "bucket";
                if !bucket.is_empty() {
                    params.put("bucket", bucket);
                }
                params.put("stats", &stats);
                if !user.is_empty() {
                    params.put("uid", &user);
                }
            },
            "unlink" => {
                if bucket.is_empty() {
                    error += &format!("Bucket value must be valid for link command. ");
                }
                if user.is_empty() {
                    error += &format!("User value must be valid for link command. ");
                }
                if !error.is_empty() {
                    let e = S3Error::new(error);
                    println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
                    return Err(e);
                }
                method = "POST".to_string();

                path += "bucket";
                params.put("bucket", bucket);
                params.put("uid", &user);
            },
            _ => {
                path += "bucket";
            }
        }

        let mut request = AdminRequest::default();
        request.bucket = Some(bucket.to_string());
        request.method = Some(method);
        request.admin_path = Some(path);
        if path_options.is_some() {
            request.path_options = path_options;
        }
        request.params = params;
        if !user.is_empty() {
            request.uid = Some(user);
        }

        match client.s3client.admin(&request) {
            Ok(output) => {
                match client.output.format {
                    OutputFormat::Serialize => {
                        // Could have already been serialized before being passed to this function.
                        println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output.payload);
                    },
                    OutputFormat::Plain => {
                        // Could have already been serialized before being passed to this function.
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                    },
                    OutputFormat::JSON => {
                        if output.format == AdminOutputType::Json {
                            println_color_quiet!(client.is_quiet,
                                                 client.output.color,
                                                 "{}",
                                                 output.payload);
                        }
                        else {
                            println_color_quiet!(client.is_quiet,
                                                 client.output.color,
                                                 "{}",
                                                 json::encode(&output.payload).unwrap_or("{}".to_string()));
                        }
                    },
                    OutputFormat::PrettyJSON => {
                        if output.format == AdminOutputType::Json {
                            println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                        }
                        else {
                            println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output.payload));
                        }
                    },
                    _ => {},
                }
            },
            Err(e) => {
                let format = format!("{:#?}", e);
                let error = S3Error::new(format);
                println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", error);
                return Err(error);
            },
        }
    }

    Ok(())
}

fn objects<P, D>(matches: &ArgMatches, bucket: &str, object: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let is_admin = client.is_admin;

    if is_admin {
        let mut method = String::from("DELETE");

        let mut command = matches.value_of("command").unwrap_or("");
        match command.clone().trim() {
            "" | "." | "*" | "$" | "s3://" => { command = ""; },
            a @ _ => { command = a; },
        }

        let mut path: String = "admin/bucket".to_string();
        let mut params = Params::new();
        let mut error: String = "".to_string();

        match command {
            "delete" => {
                if bucket.is_empty() {
                    error += &format!("Bucket value must be valid for delete command. ");
                }
                if object.is_empty() {
                    error += &format!("Object value must be valid for delete command. ");
                }
                if !error.is_empty() {
                    let e = S3Error::new(error);
                    println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
                    return Err(e);
                }
                params.put("bucket", bucket);
                params.put("object", object);
            },
            a @ _ => {
                let e = S3Error::new(format!("Invalid object command: {}", a));
                println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
                return Err(e);
            }
        }

        let mut request = AdminRequest::default();
        request.bucket = Some(bucket.to_string());
        request.method = Some(method);
        request.admin_path = Some(path);
        request.path_options = Some("?object&".to_string());
        request.params = params;

        match client.s3client.admin(&request) {
            Ok(output) => {
                match client.output.format {
                    OutputFormat::Serialize => {
                        // Could have already been serialized before being passed to this function.
                        println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output.payload);
                    },
                    OutputFormat::Plain => {
                        // Could have already been serialized before being passed to this function.
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                    },
                    OutputFormat::JSON => {
                        if output.format == AdminOutputType::Json {
                            println_color_quiet!(client.is_quiet,
                                                 client.output.color,
                                                 "{}",
                                                 output.payload);
                        }
                        else {
                            println_color_quiet!(client.is_quiet,
                                                 client.output.color,
                                                 "{}",
                                                 json::encode(&output.payload).unwrap_or("{}".to_string()));
                        }
                    },
                    OutputFormat::PrettyJSON => {
                        if output.format == AdminOutputType::Json {
                            println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                        }
                        else {
                            println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output.payload));
                        }
                    },
                    _ => {},
                }
            },
            Err(error) => {
                let format = format!("{:#?}", error);
                let error = S3Error::new(format);
                println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", error);
                return Err(error);
            },
        }
    }

    Ok(())
}

fn quota<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let is_admin = client.is_admin;

    if is_admin {
        let mut params = Params::new();
        let mut method = String::new();

        let mut user = matches.value_of("user").unwrap_or("").to_string();
        // Make into macro...
        match user.clone().trim() {
            "" | "." | "*" | "$" | "s3://" => { user = "".to_string(); },
            a @ _ => { user = a.to_string(); },
        }
        if user.is_empty() {
            let error = S3Error::new("User was not specified");
            println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", error);
            return Err(error);
        }

        params.put("uid", &user);

        let mut command = matches.value_of("command").unwrap_or("");
        match command.clone().trim() {
            "user" => {
                command = "user";
                params.put("quota-type", "user");
            },
            _ => {
                command = "bucket";
                params.put("quota-type", "bucket");
            },
        }
        let mut action = matches.value_of("action").unwrap_or("get").to_string().to_lowercase();
        match &action.clone() as &str {
            "set" => { method = "PUT".to_string(); },
            "enable" => {
                method = "PUT".to_string();
                params.put("enabled", "true");
            },
            "disable" => {
                method = "PUT".to_string();
                params.put("enabled", "false");
            },
            _ => { method = "GET".to_string(); },
        }

        if action.clone() == "set".to_string() {
            let mut size_str = matches.value_of("size").unwrap_or("").to_string();
            match &size_str.clone().to_lowercase() as &str {
                "" | "." | "*" | "$" | "s3://" => {},
                a @ _ => {
                    size_str = a.to_string();
                    if size_str == "0".to_string() {
                        size_str = "-1".to_string();
                    }
                    params.put("max-size-kb", &size_str);
                },
            }

            let mut object_str = matches.value_of("count").unwrap_or("").to_string();
            match &object_str.clone().to_lowercase() as &str {
                "" | "." | "*" | "$" | "s3://" => {},
                a @ _ => {
                    object_str = a.to_string();
                    if object_str == "0".to_string() {
                        object_str = "-1".to_string();
                    }
                    params.put("max-objects", &object_str);
                },
            }
        }

        let path: String = "admin/user".to_string();

        let mut request = AdminRequest::default();
        request.bucket = Some(bucket.to_string());
        request.method = Some(method);
        request.admin_path = Some(path);
        request.path_options = Some("?quota&".to_string());
        request.params = params;

        match client.s3client.admin(&request) {
            Ok(output) => {
                match client.output.format {
                    OutputFormat::Serialize => {
                        // Could have already been serialized before being passed to this function.
                        println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output.payload);
                    },
                    OutputFormat::Plain => {
                        // Could have already been serialized before being passed to this function.
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                    },
                    OutputFormat::JSON => {
                        if output.format == AdminOutputType::Json {
                            println_color_quiet!(client.is_quiet,
                                                 client.output.color,
                                                 "{}",
                                                 output.payload);
                        }
                        else {
                            println_color_quiet!(client.is_quiet,
                                                 client.output.color,
                                                 "{}",
                                                 json::encode(&output.payload).unwrap_or("{}".to_string()));
                        }
                    },
                    OutputFormat::PrettyJSON => {
                        if output.format == AdminOutputType::Json {
                            println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                        }
                        else {
                            println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output.payload));
                        }
                    },
                    _ => {},
                }
            },
            Err(error) => {
                let format = format!("{:#?}", error);
                let error = S3Error::new(format);
                println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", error);
                return Err(error);
            },
        }
    }

    Ok(())
}

fn caps<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let is_admin = client.is_admin;

    if is_admin {
        let mut method = String::from("PUT");
        let mut path: String = "admin/user".to_string();
        let params: Params;

        let sub_params = match matches.subcommand() {
            ("create", Some(sub_matches)) => {
                user_caps(sub_matches, bucket, &client)
            },
            ("delete", Some(sub_matches)) => {
                method = "DELETE".to_string();
                user_caps(sub_matches, bucket, &client)
            },
            (_, _) => { Err(S3Error::new("Unrecognized command")) },
        };

        match sub_params {
            Ok(subparams) => params = subparams.unwrap(),
            Err(e) => {
                let error = S3Error::new(format!("{}", e));
                println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
                return Err(error);
            },
        }

        let mut request = AdminRequest::default();
        request.bucket = Some(bucket.to_string());
        request.method = Some(method);
        request.admin_path = Some(path);
        request.path_options = Some("?caps&".to_string());
        request.params = params;

        match client.s3client.admin(&request) {
            Ok(output) => {
                match client.output.format {
                    OutputFormat::Serialize => {
                        // Could have already been serialized before being passed to this function.
                        println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output.payload);
                    },
                    OutputFormat::Plain => {
                        // Could have already been serialized before being passed to this function.
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                    },
                    OutputFormat::JSON => {
                        if output.format == AdminOutputType::Json {
                            println_color_quiet!(client.is_quiet,
                                                 client.output.color,
                                                 "{}",
                                                 output.payload);
                        }
                        else {
                            println_color_quiet!(client.is_quiet,
                                                 client.output.color,
                                                 "{}",
                                                 json::encode(&output.payload).unwrap_or("{}".to_string()));
                        }
                    },
                    OutputFormat::PrettyJSON => {
                        if output.format == AdminOutputType::Json {
                            println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                        }
                        else {
                            println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output.payload));
                        }
                    },
                    _ => {},
                }
            },
            Err(error) => {
                let format = format!("{:#?}", error);
                let error = S3Error::new(format);
                println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
                return Err(error);
            },
        }
    }

    Ok(())
}

fn user_caps<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<Option<Params>, S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut params = Params::new();

    let mut user = matches.value_of("user").unwrap_or("");
    match user.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { return Err(S3Error::new("User was not specified")); },
        a @ _ => {
            user = a;
            params.put("uid", user);
        },
    }

    let mut caps = matches.value_of("caps").unwrap_or("");
    match caps.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { return Err(S3Error::new("Caps was not specified")); },
        a @ _ => {
            caps = a;
            params.put("user-caps", caps);
        },
    }

    Ok(Some(params))
}

fn user<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let is_admin = client.is_admin;

    if is_admin {
        let mut method = String::from("GET");
        let mut path: String = "admin/user".to_string();
        let params: Params;

        let sub_params = match matches.subcommand() {
            ("create", Some(sub_matches)) => {
                method = "PUT".to_string();
                user_create(sub_matches, bucket, &client)
            },
            ("delete", Some(sub_matches)) => {
                method = "DELETE".to_string();
                user_delete(sub_matches, bucket, &client)
            },
            ("ls", Some(sub_matches)) => {
                path = "admin/metadata/user".to_string();
                user_get_list(sub_matches, bucket, false, &client)
            },
            ("modify", Some(sub_matches)) => {
                method = "POST".to_string();
                user_modify(sub_matches, bucket, &client)
            },
            (_, Some(sub_matches)) => user_get_list(sub_matches, bucket, true, &client),
            (_, None) => { Err(S3Error::new("Unrecognized command")) },
        };

        match sub_params {
            Ok(subparams) => params = subparams.unwrap(),
            Err(e) => {
                let error = S3Error::new(format!("{}", e));
                println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
                return Err(error);
            },
        }

        let mut request = AdminRequest::default();
        request.bucket = Some(bucket.to_string());
        request.method = Some(method);
        request.admin_path = Some(path);
        request.params = params;

        match client.s3client.admin(&request) {
            Ok(output) => {
                match client.output.format {
                    OutputFormat::Serialize => {
                        // Could have already been serialized before being passed to this function.
                        println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output.payload);
                    },
                    OutputFormat::Plain => {
                        // Could have already been serialized before being passed to this function.
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                    },
                    OutputFormat::JSON => {
                        if output.format == AdminOutputType::Json {
                            println_color_quiet!(client.is_quiet,
                                                 client.output.color,
                                                 "{}",
                                                 output.payload);
                        }
                        else {
                            println_color_quiet!(client.is_quiet,
                                                 client.output.color,
                                                 "{}",
                                                 json::encode(&output.payload).unwrap_or("{}".to_string()));
                        }
                    },
                    OutputFormat::PrettyJSON => {
                        if output.format == AdminOutputType::Json {
                            println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                        }
                        else {
                            println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output.payload));
                        }
                    },
                    _ => {},
                }
            },
            Err(error) => {
                let format = format!("{:#?}", error);
                let error = S3Error::new(format);
                println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
                return Err(error);
            },
        }
    }

    Ok(())
}

fn user_create<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<Option<Params>, S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut params = Params::new();

    let mut user = matches.value_of("user").unwrap_or("");
    match user.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { return Err(S3Error::new("User was not specified")); },
        a @ _ => {
            user = a;
            params.put("uid", user);
        },
    }

    let mut display_name = matches.value_of("display_name").unwrap_or("");
    match display_name.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { return Err(S3Error::new("Display-name was not specified")); },
        a @ _ => {
            display_name = a;
            params.put("display-name", display_name);
        },
    }

    let mut email = matches.value_of("email").unwrap_or("");
    match email.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { email = ""; },
        a @ _ => {
            email = a;
            params.put("email", email);
        },
    }

    let mut access_key = matches.value_of("access_key").unwrap_or("");
    match access_key.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { access_key = ""; },
        a @ _ => {
            access_key = a;
            params.put("access-key", access_key);
        },
    }

    let mut secret_key = matches.value_of("secret_key").unwrap_or("");
    match secret_key.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { secret_key = ""; },
        a @ _ => {
            secret_key = a;
            params.put("secret-key", secret_key);
        },
    }

    let mut suspended = matches.value_of("suspended").unwrap_or("false");
    match suspended.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { suspended = "false"; },
        a @ _ => {
            suspended = a;
            params.put("suspended", suspended);
        },
    }

    let mut caps = matches.value_of("caps").unwrap_or("");
    match caps.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { caps = ""; },
        a @ _ => {
            caps = a;
            params.put("caps", caps);
        },
    }

    Ok(Some(params))
}

fn user_delete<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<Option<Params>, S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut user = matches.value_of("user").unwrap_or("");
    match user.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { user = ""; },
        a @ _ => { user = a; },
    }

    if user.is_empty() {
        return Err(S3Error::new("User was not specified"));
    }

    let mut purge_data = matches.value_of("purge_data").unwrap_or("true");
    match purge_data.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { purge_data = "true"; },
        a @ _ => { purge_data = a; },
    }

    let mut params = Params::new();
    params.put("uid", user);
    params.put("purge-data", purge_data);

    Ok(Some(params))
}

fn user_get_list<P, D>(matches: &ArgMatches, bucket: &str, user_required: bool, client: &Client<P, D>) -> Result<Option<Params>, S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut user = matches.value_of("user").unwrap_or("");
    match user.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { user = ""; },
        a @ _ => { user = a; },
    }

    if user.is_empty() && user_required {
        return Err(S3Error::new("User was not specified"));
    }

    let mut params = Params::new();
    params.put("uid", user);

    Ok(Some(params))
}

fn user_modify<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<Option<Params>, S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut params = Params::new();

    let mut user = matches.value_of("user").unwrap_or("");
    match user.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { return Err(S3Error::new("User was not specified")); },
        a @ _ => {
            user = a;
            params.put("uid", user);
        },
    }

    let mut display_name = matches.value_of("display_name").unwrap_or("");
    match display_name.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { display_name = ""; },
        a @ _ => {
            display_name = a;
            params.put("display-name", display_name);
        },
    }

    let mut suspended = matches.value_of("suspended").unwrap_or("false");
    match suspended.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { suspended = "false"; },
        a @ _ => {
            suspended = a;
            params.put("suspended", suspended);
        },
    }

    let mut email = matches.value_of("email").unwrap_or("");
    match email.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { email = ""; },
        a @ _ => {
            email = a;
            params.put("email", email);
        },
    }

    let mut access_key = matches.value_of("access_key").unwrap_or("");
    match access_key.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { access_key = ""; },
        a @ _ => {
            access_key = a;
            params.put("access-key", access_key);
        },
    }

    let mut secret_key = matches.value_of("secret_key").unwrap_or("");
    match secret_key.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { secret_key = ""; },
        a @ _ => {
            secret_key = a;
            params.put("secret-key", secret_key);
        },
    }

    let mut caps = matches.value_of("caps").unwrap_or("");
    match caps.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { caps = ""; },
        a @ _ => {
            caps = a;
            params.put("caps", caps);
        },
    }

    let mut max = matches.value_of("max_buckets").unwrap_or("");
    match max.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { max = ""; },
        a @ _ => {
            max = a;
            params.put("max-buckets", max);
        },
    }

    Ok(Some(params))
}

fn usage<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let is_admin = client.is_admin;

    if is_admin {
        let mut method = String::from("GET");
        let mut path: String = "admin/usage".to_string();
        let params: Params;

        let sub_params = match matches.subcommand() {
            ("trim", Some(sub_matches)) => {
                method = "DELETE".to_string();
                usage_trim(sub_matches, bucket, &client)
            },
            (_, Some(sub_matches)) => usage_list(sub_matches, bucket, &client),
            (_, None) => { Err(S3Error::new("Unrecognized command")) },
        };

        match sub_params {
            Ok(subparams) => params = subparams.unwrap(),
            Err(e) => {
                let error = S3Error::new(format!("{}", e));
                println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
                return Err(error);
            },
        }

        let mut request = AdminRequest::default();
        request.bucket = Some(bucket.to_string());
        request.method = Some(method);
        request.admin_path = Some(path);
        request.params = params;

        match client.s3client.admin(&request) {
            Ok(output) => {
                match client.output.format {
                    OutputFormat::Serialize => {
                        // Could have already been serialized before being passed to this function.
                        println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output.payload);
                    },
                    OutputFormat::Plain => {
                        // Could have already been serialized before being passed to this function.
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                    },
                    OutputFormat::JSON => {
                        if output.format == AdminOutputType::Json {
                            println_color_quiet!(client.is_quiet,
                                                 client.output.color,
                                                 "{}",
                                                 output.payload);
                        }
                        else {
                            println_color_quiet!(client.is_quiet,
                                                 client.output.color,
                                                 "{}",
                                                 json::encode(&output.payload).unwrap_or("{}".to_string()));
                        }
                    },
                    OutputFormat::PrettyJSON => {
                        if output.format == AdminOutputType::Json {
                            println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                        }
                        else {
                            println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output.payload));
                        }
                    },
                    _ => {},
                }
            },
            Err(error) => {
                let format = format!("{:#?}", error);
                let error = S3Error::new(format);
                println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", error);
                return Err(error);
            },
        }
    }

    Ok(())
}

fn usage_list<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<Option<Params>, S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut params = Params::new();

    let mut user = matches.value_of("user").unwrap_or("");
    match user.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => {
            user = "";
        },
        a @ _ => {
            user = a;
            params.put("uid", user);
        },
    }

    let mut start = matches.value_of("start").unwrap_or("");
    match start.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { start = ""; },
        a @ _ => {
            start = a;
            params.put("start", start);
        },
    }

    let mut end = matches.value_of("end").unwrap_or("");
    match end.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { end = ""; },
        a @ _ => {
            end = a;
            params.put("end", end);
        },
    }

    let mut show_entries = matches.value_of("show_entries").unwrap_or("false");
    match show_entries.clone().trim() {
        "true" => {
            show_entries = "true";
            params.put("show-entries", "true");
        },
        _ => {
            show_entries = "false";
        },
    }

    let mut show_summary = matches.value_of("show_summary").unwrap_or("false");
    match show_summary.clone().trim() {
        "true" => {
            show_summary = "true";
            params.put("show-summary", "true");
        },
        _ => {
            show_summary = "false";
        },
    }

    Ok(Some(params))
}

fn usage_trim<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<Option<Params>, S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut params = Params::new();

    let mut user = matches.value_of("user").unwrap_or("");
    match user.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => {
            user = "";
        },
        a @ _ => {
            user = a;
            params.put("uid", user);
        },
    }

    let mut start = matches.value_of("start").unwrap_or("");
    match start.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { start = ""; },
        a @ _ => {
            start = a;
            params.put("start", start);
        },
    }

    let mut end = matches.value_of("end").unwrap_or("");
    match end.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { end = ""; },
        a @ _ => {
            end = a;
            params.put("end", end);
        },
    }

    let mut remove_all = matches.value_of("remove_all").unwrap_or("true");
    match remove_all.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => {
            remove_all = "true";
        },
        a @ _ => {
            remove_all = a;
        },
    }

    if !user.is_empty() {
        remove_all = "true";
    }
    params.put("remove-all", remove_all);

    Ok(Some(params))
}

fn keys<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let is_admin = client.is_admin;

    if is_admin {
        let mut method = String::from("GET");
        let mut path: String = "admin/user".to_string();
        let params: Params;

        let sub_params = match matches.subcommand() {
            ("create", Some(sub_matches)) => {
                method = "PUT".to_string();
                user_key_create(sub_matches, bucket, &client)
            },
            ("delete", Some(sub_matches)) => {
                method = "DELETE".to_string();
                user_key_delete(sub_matches, bucket, &client)
            },
            (_, _) => { Err(S3Error::new("Unrecognized command")) },
        };

        match sub_params {
            Ok(subparams) => params = subparams.unwrap(),
            Err(e) => {
                let error = S3Error::new(format!("{}", e));
                println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
                return Err(error);
            },
        }

        let mut request = AdminRequest::default();
        request.bucket = Some(bucket.to_string());
        request.method = Some(method);
        request.admin_path = Some(path);
        request.path_options = Some("?key&".to_string());
        request.params = params;

        match client.s3client.admin(&request) {
            Ok(output) => {
                match client.output.format {
                    OutputFormat::Serialize => {
                        // Could have already been serialized before being passed to this function.
                        println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output.payload);
                    },
                    OutputFormat::Plain => {
                        // Could have already been serialized before being passed to this function.
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                    },
                    OutputFormat::JSON => {
                        if output.format == AdminOutputType::Json {
                            println_color_quiet!(client.is_quiet,
                                                 client.output.color,
                                                 "{}",
                                                 output.payload);
                        }
                        else {
                            println_color_quiet!(client.is_quiet,
                                                 client.output.color,
                                                 "{}",
                                                 json::encode(&output.payload).unwrap_or("{}".to_string()));
                        }
                    },
                    OutputFormat::PrettyJSON => {
                        if output.format == AdminOutputType::Json {
                            println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                        }
                        else {
                            println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output.payload));
                        }
                    },
                    _ => {},
                }
            },
            Err(error) => {
                let format = format!("{:#?}", error);
                let error = S3Error::new(format);
                println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
                return Err(error);
            },
        }
    }

    Ok(())
}

fn user_key_create<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<Option<Params>, S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut params = Params::new();

    let mut user = matches.value_of("user").unwrap_or("");
    match user.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { return Err(S3Error::new("User was not specified")); },
        a @ _ => {
            user = a;
            params.put("uid", user);
        },
    }

    let mut generate_key = matches.value_of("generate_key").unwrap_or("");
    match generate_key.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { generate_key = "true"; },
        a @ _ => {
            generate_key = a;
            params.put("generate-key", generate_key);
        },
    }

    let mut access_key = matches.value_of("access_key").unwrap_or("");
    match access_key.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { access_key = "" },
        a @ _ => {
            access_key = a;
            params.put("access-key", access_key);
        },
    }

    let mut secret_key = matches.value_of("secret_key").unwrap_or("");
    match secret_key.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { secret_key = ""; },
        a @ _ => {
            secret_key = a;
            params.put("secret-key", secret_key);
        },
    }

    Ok(Some(params))
}

fn user_key_delete<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<Option<Params>, S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut params = Params::new();

    let mut access_key = matches.value_of("access_key").unwrap_or("");
    match access_key.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { return Err(S3Error::new("Access-key was not specified")); },
        a @ _ => {
            access_key = a;
            params.put("access-key", access_key);
        },
    }

    let mut user = matches.value_of("user").unwrap_or("");
    match user.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { user = ""; },
        a @ _ => {
            user = a;
            params.put("uid", user);
        },
    }

    Ok(Some(params))
}
