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

/// All GET requests pass through this function.
pub fn commands<P, D>(matches: &ArgMatches,
                      client: &mut Client<P,D>)
                      -> Result<(), S3Error>
                      where P: AwsCredentialsProvider,
                            D: DispatchSignedRequest {
    let bucket = matches.value_of("name").unwrap_or("");

    match matches.subcommand() {
        /// acl command.
        ("acl", _) => {
            // Will bubble error up via try!
            let acl = try!(get_bucket_acl(bucket, client));
            match client.output.format {
                OutputFormat::Plain => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", acl);
                },
                OutputFormat::JSON => {
                    println_color_quiet!(client.is_quiet, client.output.color, "{}", json::encode(&acl).unwrap_or("{}".to_string()));
                },
                OutputFormat::PrettyJSON => {
                    println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&acl));
                },
                OutputFormat::None => {},
                e @ _ => println_color_quiet!(client.is_quiet, client.error.color, "Error: Format - {:#?}", e),
            }
        },
        ("head", _) => {
            let list = try!(get_bucket_head(bucket, client));
        },
        ("list", _) => {
            let list = try!(get_buckets_list(client));
        },
        ("versioning", _) => {
            let list = try!(get_bucket_versioning(bucket, client));
        },
        (e,_) => {
            //if e.is_empty() && !bucket.is_empty(){
                // Lists objects of a given bucket
            //    let output = get_bucket_list(bucket, client);
            //} else {
                let error = format!("incorrect or missing request {}", e);
                println_color_quiet!(client.is_quiet, client.error.color, "{:?}", error);
            //}
        }
    }

    Ok(())
}

fn get_bucket_head<P, D>(bucket: &str,
                         client: &Client<P,D>)
                         -> Result<(), S3Error>
                         where P: AwsCredentialsProvider,
                               D: DispatchSignedRequest {
     let request = HeadBucketRequest { bucket: bucket.to_string() };

     match client.s3client.head_bucket(&request) {
         Ok(_) => {
           println_color_quiet!(client.is_quiet, client.output.color, "Bucket exists");
           Ok(())
         },
         Err(e) => {
           println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
           Err(e)
         },
     }
}

fn get_bucket_versioning<P, D>(bucket: &str,
                               client: &Client<P,D>)
                               -> Result<(), S3Error>
                               where P: AwsCredentialsProvider,
                                     D: DispatchSignedRequest {
     let request = GetBucketVersioningRequest { bucket: bucket.to_string() };

     match client.s3client.get_bucket_versioning(&request) {
         Ok(version) => {
           println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", version);
           Ok(())
         },
         Err(e) => {
           println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
           Err(e)
         },
     }
}

fn get_buckets_list<P, D>(client: &Client<P,D>)
                          -> Result<(), S3Error>
                          where P: AwsCredentialsProvider,
                                D: DispatchSignedRequest {
    match client.s3client.list_buckets() {
      Ok(output) => {
          match client.output.format {
              OutputFormat::Plain => {
                  // Could have already been serialized before being passed to this function.
                  println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
              },
              OutputFormat::JSON => {
                  println_color_quiet!(client.is_quiet, client.output.color, "{}", json::encode(&output).unwrap_or("{}".to_string()));
              },
              OutputFormat::PrettyJSON => {
                  println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output));
              },
              OutputFormat::None => {},
              e @ _ => println_color_quiet!(client.is_quiet, client.error.color, "Error: Format - {:#?}", e),
          }
          Ok(())
      }
      Err(error) => {
          let format = format!("{:#?}", error);
          let error = S3Error::new(format);
          println_color_quiet!(client.is_quiet, client.error.color, "{:?}", error);
          Err(error)
      }
    }
}

pub fn get_bucket_acl<P, D>(bucket: &str,
                            client: &Client<P,D>)
                            -> Result<AccessControlPolicy, S3Error>
                            where P: AwsCredentialsProvider,
                                  D: DispatchSignedRequest {
    let mut request = GetBucketAclRequest::default();
    request.bucket = bucket.to_string();

    match client.s3client.get_bucket_acl(&request) {
        Ok(acl) => Ok(acl),
        Err(e) => {
            println_color_quiet!(client.is_quiet, client.error.color, "{:?}", e);
            Err(e)
        }
    }
}
