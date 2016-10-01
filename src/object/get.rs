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

use std::fs::File;
use std::io::Write;

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
                      bucket: &str,
                      client: &mut Client<P,D>)
                      -> Result<(), S3Error>
                      where P: AwsCredentialsProvider,
                            D: DispatchSignedRequest {
  let object = matches.value_of("object").unwrap_or("");

  match matches.subcommand() {
    /// acl command.
    ("acl", _) => {
      let acl = try!(get_object_acl(bucket, object, client));
    },
    ("head", _) => {
      let acl = try!(get_object_head(bucket, object, client));
    },
    (e,_) => {
      if e.is_empty() {
        let mut path = matches.value_of("path").unwrap_or("");

        if path.is_empty() {
          path = object;
        }

        let result = get_object(bucket, object, path, client);
      } else {
        let error = format!("incorrect or missing request {}", e);
        println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
        return Err(S3Error::new(error));
      }
    }
  }

  Ok(())
}

// Limited in file size.
fn get_object<P, D>(bucket: &str,
                    object: &str,
                    path: &str,
                    client: &Client<P,D>)
                    -> Result<(), S3Error>
                    where P: AwsCredentialsProvider,
                          D: DispatchSignedRequest {
   let mut request = GetObjectRequest::default();
    request.bucket = bucket.to_string();
    request.key = object.to_string();

   match client.s3client.get_object(&request) {
     Ok(output) => {
       let mut file = File::create(path).unwrap();
       match file.write_all(&output.body) {
         Ok(_) => {
           println_color_quiet!(client.is_quiet, client.output.color, "Success");
           Ok(())
         },
         Err(e) => {
           let error = format!("{:#?}", e);
           println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
           Err(S3Error::new(error))
         }
       }
     },
     Err(e) => {
       println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
       Err(e)
     },
   }
}

fn get_object_head<P, D>(bucket: &str,
                         object: &str,
                         client: &Client<P,D>)
                         -> Result<(), S3Error>
                         where P: AwsCredentialsProvider,
                               D: DispatchSignedRequest {
   let mut request = HeadObjectRequest::default();
   request.bucket = bucket.to_string();
   request.key = object.to_string();

   match client.s3client.head_object(&request) {
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

fn get_object_acl<P, D>(bucket: &str,
                        object: &str,
                        client: &Client<P, D>)
                        -> Result<(), S3Error>
                        where P: AwsCredentialsProvider,
                              D: DispatchSignedRequest {
  let mut request = GetObjectAclRequest::default();
  request.bucket = bucket.to_string();
  request.key = object.to_string();

  match client.s3client.get_object_acl(&request) {
    Ok(acl) => {
      println_color_quiet!(client.is_quiet, client.output.color, "{:?}", acl);
      Ok(())
    },
    Err(e) => {
      let format = format!("{:#?}", e);
      println_color_quiet!(client.is_quiet, client.error.color, "{:?}", e);
      Err(e)
    }
  }
}
