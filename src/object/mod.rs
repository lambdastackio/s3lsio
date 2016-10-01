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
#![allow(unused_must_use)]
#![allow(unused_variables)]

use clap::ArgMatches;
use term;

use aws_sdk_rust::aws::errors::s3::S3Error;
use aws_sdk_rust::aws::common::credentials::AwsCredentialsProvider;
use aws_sdk_rust::aws::common::request::DispatchSignedRequest;
use aws_sdk_rust::aws::s3::object::*;

use Client;
use Output;

pub mod get;
pub mod put;
pub mod delete;

pub fn commands<P, D>(matches: &ArgMatches,
                      client: &mut Client<P,D>)
                      -> Result<(), S3Error>
                      where P: AwsCredentialsProvider,
                            D: DispatchSignedRequest {
    let bucket = matches.value_of("bucket").unwrap_or("");

    match matches.subcommand() {
        ("get", Some(sub_matches)) => {
            match get::commands(sub_matches, bucket, client) {
                Err(e) => Err(e),
                Ok(_) => Ok(())
            }
        },
        ("list", _) => {
          if bucket.is_empty() {
            let error = format!("missing bucket name");
            println_color_quiet!(client.is_quiet, term::color::RED, "{}", error);
            Err(S3Error::new(error))
          } else {
            let acl = try!(get_bucket_list(bucket, client));
            Ok(())
          }
        },
        ("put", Some(sub_matches)) => {
          match put::commands(sub_matches, bucket, client) {
              Err(e) => Err(e),
              Ok(_) => Ok(())
          }
        },
        ("delete", Some(sub_matches)) => {
          match delete::commands(sub_matches, bucket, client) {
            Err(e) => Err(e),
            Ok(_) => Ok(())
          }
        },
        (e, _) => {
          let error = format!("incorrect or missing request {}", e);
          println_color_quiet!(client.is_quiet, term::color::RED, "{}", error);
          return Err(S3Error::new(error));
        },
    };

    Ok(())
}

fn get_bucket_list<P, D>(bucket: &str,
                         client: &Client<P,D>)
                         -> Result<(), S3Error>
                         where P: AwsCredentialsProvider,
                               D: DispatchSignedRequest {
    let mut request = ListObjectsRequest::default();
    request.bucket = bucket.to_string();
    request.version = Some(2);

    match client.s3client.list_objects(&request) {
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
