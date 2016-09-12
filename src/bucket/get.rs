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
//

#![allow(unused_imports)]
#![allow(unused_variables)]

//! GET verb
//!
//! All GET requests are handled in this module.
//!

use rustc_serialize::json;

use clap::ArgMatches;
use aws_sdk_rust::aws::errors::s3::S3Error;
use aws_sdk_rust::aws::common::credentials::AwsCredentialsProvider;
use aws_sdk_rust::aws::common::request::DispatchSignedRequest;
use aws_sdk_rust::aws::s3::acl::*;
use aws_sdk_rust::aws::s3::bucket::*;
use aws_sdk_rust::aws::s3::object::ListObjectsRequest;

use term;
use Client;
use Output;
use OutputFormat;
use util::*;

/// All GET requests pass through this function.
pub fn commands<P: AwsCredentialsProvider, D: DispatchSignedRequest>(matches: &ArgMatches, client: &mut Client<P,D>) -> Result<(), S3Error> {
    //println!("Bucket-get -- get::commands::{:#?}", matches);
    let bucket = matches.value_of("name").unwrap_or("");

    match matches.subcommand() {
        /// acl command.
        ("acl", _) => {
            // Will bubble error up via try!
            let acl = try!(get_bucket_acl(bucket, client));
            match client.output.format {
                OutputFormat::Plain => {
                    // Could have already been serialized before being passed to this function.
                    println_color!(client.output.color, "{:#?}", acl);
                },
                OutputFormat::JSON => {
                    println_color!(client.output.color, "{}", json::as_pretty_json(&acl));
                },
                OutputFormat::None => {},
                _ => println!("error"),
            }

        },
        ("list", _) => {
            // Will bubble error up via try!
            let list = try!(get_buckets_list(client));
        },
        (e,_) => {
            if e.is_empty() && !bucket.is_empty(){
                // Lists objects of a given bucket
                let output = get_bucket_list(bucket, client);
            } else {
                let error = format!("incorrect or missing request {}", e);
                println_color!(term::color::RED, "{}", error);
            }
        }
    }

    Ok(())
}

fn get_buckets_list<P: AwsCredentialsProvider, D: DispatchSignedRequest>(client: &Client<P,D>) -> Result<(), S3Error> {
    match client.s3client.list_buckets() {
      Ok(output) => {
          let format = format!("{:#?}", output);
          print_output(&client.output, &format);
          Ok(())
      }
      Err(error) => {
          let format = format!("{:#?}", error);
          print_error(&client.error, &format);
          let error = S3Error::new(format);
          Err(error)
      }
    }
}

fn get_bucket_list<P: AwsCredentialsProvider, D: DispatchSignedRequest>(bucket: &str, client: &Client<P,D>) -> Result<(), S3Error> {
    let mut list_objects = ListObjectsRequest::default();
    list_objects.bucket = bucket.to_string();

    match client.s3client.list_objects(&list_objects) {
      Ok(output) => {
          let format = format!("{:#?}", output);
          print_output(&client.output, &format);
          Ok(())
      }
      Err(error) => {
          let format = format!("{:#?}", error);
          print_error(&client.error, &format);
          let error = S3Error::new(format);
          Err(error)
      }
    }
}

pub fn get_bucket_acl<P: AwsCredentialsProvider, D: DispatchSignedRequest>(bucket: &str, client: &Client<P,D>) -> Result<AccessControlPolicy, S3Error> {
    let mut get_bucket_acl = GetBucketAclRequest::default();
    get_bucket_acl.bucket = bucket.to_string();

    match client.s3client.get_bucket_acl(&get_bucket_acl) {
        Ok(acl) => Ok(acl),
        Err(e) => {
            let format = format!("{:#?}", e);
            println_color!(term::color::RED, "missing or incorrect bucket name");
            print_error(&client.error, &format);
            Err(e)
        }
    }
}

fn print_acl_output(acl: &AccessControlPolicy, output: &Output) -> Result<(), S3Error> {
    println!("{:#?}", acl);

    Ok(())
}
