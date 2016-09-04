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

//! GET verb
//!
//! All GET requests are handled in this module.
//!

use clap::ArgMatches;
use aws_sdk_rust::aws::errors::s3::S3Error;
use aws_sdk_rust::aws::common::credentials::AwsCredentialsProvider;
use aws_sdk_rust::aws::common::request::DispatchSignedRequest;
use aws_sdk_rust::aws::s3::acl::*;
use aws_sdk_rust::aws::s3::bucket::*;

use Client;
use Output;
use util::*;

/// All GET requests pass through this function.
pub fn commands<P: AwsCredentialsProvider, D: DispatchSignedRequest>(matches: &ArgMatches, client: &mut Client<P,D>) -> Result<(), S3Error> {
    //println!("Bucket-get -- get::commands::{:#?}", matches);
    let bucket = matches.value_of("name").unwrap_or("");

    match matches.subcommand() {
        /// acl command.
        ("acl", _) => {
            let acl = get_bucket_acl(bucket, client);
            if let Ok(acl) = acl {
                print_acl_output(&acl, &client.output);
            } else if let Err(acl) = acl {
                //println!("{:#?}", acl);
            }
        }
        (e,_) => {
            if e.is_empty() && bucket.is_empty() {
                get_bucket_list(client);
            } else {
                let error = format!("incorrect or missing request {}", e);
                println!("{}", error);
                //Err(S3Error::new(error))
            }
        }
    }

    Ok(())
}

fn get_bucket_list<P: AwsCredentialsProvider, D: DispatchSignedRequest>(client: &Client<P,D>) -> Result<(), S3Error> {
    match client.s3client.list_buckets() {
      Ok(output) => {
          let format = format!("{:#?}", output);
          print_output(&client.output, &format);
          Ok(())
      }
      Err(error) => {
          let format = format!("{:#?}", error);
          let error = S3Error::new(format);
          print_error(&client.error, &error);
          Err(error)
      }
    }
}

fn get_bucket_acl<P: AwsCredentialsProvider, D: DispatchSignedRequest>(bucket: &str, client: &Client<P,D>) -> Result<AccessControlPolicy, S3Error> {
    let mut get_bucket_acl = GetBucketAclRequest::default();
    get_bucket_acl.bucket = bucket.to_string();

    match client.s3client.get_bucket_acl(&get_bucket_acl) {
        Ok(acl) => Ok(acl),
        Err(e) => {
            print_error(&client.error, &e);
            Err(e)
        }
    }
}

fn print_acl_output(acl: &AccessControlPolicy, output: &Output) -> Result<(), S3Error> {
    println!("{:#?}", acl);

    Ok(())
}
