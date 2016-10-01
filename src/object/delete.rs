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

use term;
use clap::ArgMatches;
use aws_sdk_rust::aws::errors::s3::S3Error;
use aws_sdk_rust::aws::common::credentials::AwsCredentialsProvider;
use aws_sdk_rust::aws::common::request::DispatchSignedRequest;
use aws_sdk_rust::aws::s3::object::*;

use Client;
use Output;

pub fn commands<P, D>(matches: &ArgMatches,
                      bucket: &str,
                      client: &mut Client<P,D>)
                      -> Result<(), S3Error>
                      where P: AwsCredentialsProvider,
                            D: DispatchSignedRequest {
  let object = matches.value_of("object").unwrap_or("");
  let version = matches.value_of("version").unwrap_or("");

  //NOTE: For now there is only one delete function, deleting the object

  let result = delete_object(bucket, object, version, client);

  Ok(())
}

fn delete_object<P, D>(bucket: &str,
                       object: &str,
                       version: &str,
                       client: &Client<P, D>)
                       -> Result<(), S3Error>
                       where P: AwsCredentialsProvider,
                             D: DispatchSignedRequest {
   let mut request = DeleteObjectRequest::default();
   request.bucket = bucket.to_string();
   request.key = object.to_string();
   if !version.is_empty() {
     request.version_id = Some(version.to_string());
   }

   match client.s3client.delete_object(&request) {
       Ok(output) => println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output),
       Err(e) => println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e),
   }

  Ok(())
}
