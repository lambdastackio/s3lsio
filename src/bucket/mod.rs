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

use term;
use rustc_serialize::json;

use clap::ArgMatches;
use aws_sdk_rust::aws::errors::s3::S3Error;
use aws_sdk_rust::aws::common::credentials::AwsCredentialsProvider;
use aws_sdk_rust::aws::common::request::DispatchSignedRequest;
use aws_sdk_rust::aws::s3::bucket::*;

use Client;
use Output;
use OutputFormat;

pub mod get;
pub mod put;
pub mod delete;

/// All bucket commands are passed through this function.
pub fn commands<P, D>(matches: &ArgMatches,
                      client: &mut Client<P,D>)
                      -> Result<(), S3Error>
                      where P: AwsCredentialsProvider,
                            D: DispatchSignedRequest {
    match matches.subcommand() {
        ("get", Some(sub_matches)) => {
            match get::commands(sub_matches, client) {
                Err(e) => Err(e),
                Ok(_) => Ok(())
            }
        }
        /// create new bucket
        ("create", Some(sub_matches)) => {
          let bucket = sub_matches.value_of("name").unwrap_or("");

          if bucket.is_empty() {
            let error = format!("missing bucket name");
            println_color_quiet!(client.is_quiet, term::color::RED, "{}", error);
            Err(S3Error::new(error))
          } else {
            let result = bucket_create(sub_matches, bucket, client);
            Ok(())
          }
        },
        ("list", Some(sub_matches)) => {
          let list = try!(buckets_list(client));
          Ok(())
        }
        ("put", Some(sub_matches)) => {
            match put::commands(sub_matches, client) {
                Err(e) => Err(e),
                Ok(_) => Ok(())
            }
        },
        ("delete", Some(sub_matches)) => delete::commands(sub_matches, client),
        (e, _) => {
                let error = format!("unsupported command {} - available commands are get, put, delete", e);
                println_color!(term::color::RED, "{}", error);
                Ok(())
        },
    };

    Ok(())
}

fn bucket_create<P, D>(sub_matches: &ArgMatches,
                           bucket: &str,
                           client: &Client<P, D>)
                           -> Result<(), S3Error>
                           where P: AwsCredentialsProvider,
                                 D: DispatchSignedRequest {
    let mut request = CreateBucketRequest::default();
    request.bucket = bucket.to_string();

    match client.s3client.create_bucket(&request) {
        Ok(_) => {
          println_color_quiet!(client.is_quiet, client.output.color, "Success");
          Ok(())
        },
        Err(e) => {
          println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
          Err(e)
        },
    }
}

fn buckets_list<P, D>(client: &Client<P,D>)
                          -> Result<(), S3Error>
                          where P: AwsCredentialsProvider,
                                D: DispatchSignedRequest {
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
                  println_color_quiet!(client.is_quiet, client.output.color, "{}", json::encode(&output).unwrap_or("{}".to_string()));
              },
              OutputFormat::PrettyJSON => {
                  println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output));
              },
              OutputFormat::None => {},
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
