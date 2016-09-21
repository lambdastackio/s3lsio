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
#![allow(unused_variables)]

use clap::ArgMatches;
use aws_sdk_rust::aws::errors::s3::S3Error;
use aws_sdk_rust::aws::common::credentials::AwsCredentialsProvider;
use aws_sdk_rust::aws::common::request::DispatchSignedRequest;
use aws_sdk_rust::aws::s3::acl::*;
use aws_sdk_rust::aws::s3::object::*;

use term;
use Client;
use Output;
use util::*;

// NOTE: Want an option to output only specific keys in a custom format so that users can
// specify only what they want to see in results.
// This feature would be for those that don't want to use aws-sdk-rust library and build their
// own but need something in a hurry that they can use with shell scripts etc.

pub fn commands<P, D>(matches: &ArgMatches,
                      client: &mut Client<P,D>)
                      -> Result<(), S3Error>
                      where P: AwsCredentialsProvider,
                            D: DispatchSignedRequest {
    let bucket = matches.value_of("bucket").unwrap_or("");
    let object = matches.value_of("name").unwrap_or("");

    match matches.subcommand() {
        /// acl command.
        ("acl", _) => {
            let acl = try!(get_object_acl(bucket, object, client));
        },
        ("head", _) => {
            let acl = try!(get_object_head(bucket, object, client));
        },
        ("list", _) => {
            let acl = try!(get_bucket_list(bucket, client));
        },
        (e,_) => {
            if e.is_empty() && bucket.is_empty() {
                // Lists objects
                //get_buckets_list(client);
            } else if e.is_empty() && !bucket.is_empty(){
                // Lists objects
                //get_object_list(bucket, client);
            } else {
                //let error = format!("incorrect or missing request {}", e);
                //println_color!(term::color::RED, "{}", error);
            }
        }
    }

    Ok(())
}

fn get_bucket_list<P, D>(bucket: &str,
                         client: &Client<P,D>)
                         -> Result<(), S3Error>
                         where P: AwsCredentialsProvider,
                               D: DispatchSignedRequest {
    let mut list_objects = ListObjectsRequest::default();
    list_objects.bucket = bucket.to_string();

    match client.s3client.list_objects(&list_objects) {
      Ok(output) => {
          println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
          Ok(())
      }
      Err(error) => {
          let format = format!("{:#?}", error);
          let error = S3Error::new(format);
          println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", error);
          Err(error)
      }
    }
}

fn get_object_head<P, D>(bucket: &str,
                         object: &str,
                         client: &Client<P,D>)
                         -> Result<(), S3Error>
                         where P: AwsCredentialsProvider,
                               D: DispatchSignedRequest {
     let mut head_object = HeadObjectRequest::default();
     head_object.bucket = bucket.to_string();
     head_object.key = object.to_string();

     match client.s3client.head_object(&head_object) {
         Ok(head) => {
           println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", head);
           Ok(())
         },
         Err(e) => {
           println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
           Err(e)
         },
     }
}

fn get_object_acl<P: AwsCredentialsProvider, D: DispatchSignedRequest>(bucket: &str, object: &str, client: &Client<P,D>) -> Result<AccessControlPolicy, S3Error> {
    let mut get_object_acl = GetObjectAclRequest::default();
    get_object_acl.bucket = bucket.to_string();
    get_object_acl.key = object.to_string();

    match client.s3client.get_object_acl(&get_object_acl) {
        Ok(acl) => {
          println_color_quiet!(client.is_quiet, client.output.color, "{:?}", acl);
          Ok(acl)
        },
        Err(e) => {
            let format = format!("{:#?}", e);
            println_color_quiet!(client.is_quiet, client.error.color, "{:?}", e);
            Err(e)
        }
    }
}
