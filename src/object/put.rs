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

use std::io;
use std::io::{Read, Seek, SeekFrom, BufReader};
use std::path::Path;
use std::fs::File;
use std::ffi::OsStr;

use clap::ArgMatches;
use aws_sdk_rust::aws::errors::s3::S3Error;
use aws_sdk_rust::aws::common::credentials::AwsCredentialsProvider;
use aws_sdk_rust::aws::common::request::DispatchSignedRequest;
use aws_sdk_rust::aws::s3::acl::*;
use aws_sdk_rust::aws::s3::object::*;

use term;
use Client;
use Output;

pub fn commands<P, D>(matches: &ArgMatches,
                      bucket: &str,
                      client: &mut Client<P,D>)
                      -> Result<(), S3Error>
                      where P: AwsCredentialsProvider,
                            D: DispatchSignedRequest {
  // Object name is file to be uploaded.
  let object = matches.value_of("object").unwrap_or("");

  match matches.subcommand() {
    ("key", Some(sub_matches)) => {
      // Key will set the object name "key"
      let key = sub_matches.value_of("key").unwrap_or("");
      let result = put_object(bucket, key, object, client);
    },
    (e, _) => {
      if e.is_empty() {
        // This will assume you want to upload the given object and make the file name the key
        let path = Path::new(object);
        let key = path.file_name().unwrap().to_str().unwrap();

        let result = put_object(bucket, key, object, client);
      } else {
        let error = format!("incorrect or missing request {}", e);
        println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
        return Err(S3Error::new(error));
      }
    },
  }

  Ok(())
}

// Limited in file size. Max is 5GB but should use Multipart upload for larger than 15MB.
fn put_object<P, D>(bucket: &str,
                    key: &str,
                    object: &str,
                    client: &Client<P, D>)
                    -> Result<(), S3Error>
                    where P: AwsCredentialsProvider,
                          D: DispatchSignedRequest {
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

  let mut request = PutObjectRequest::default();
  request.bucket = bucket.to_string();
  request.key = key.to_string();
  request.body = Some(&buffer);

  match client.s3client.put_object(&request) {
      Ok(output) => {
        println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
        Ok(())
      },
      Err(e) => {
        let error = format!("{:#?}", e);

        println!("{:?}", request);

        println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
        Err(S3Error::new(error))
      },
  }
}
