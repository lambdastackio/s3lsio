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

use clap::ArgMatches;
use aws_sdk_rust::aws::errors::s3::S3Error;
use aws_sdk_rust::aws::common::credentials::AwsCredentialsProvider;
use aws_sdk_rust::aws::common::request::DispatchSignedRequest;
use aws_sdk_rust::aws::s3::acl::*;
use aws_sdk_rust::aws::s3::bucket::*;

use term;
use Client;
use Output;
use OutputFormat;
use util::*;
use bucket::get::get_bucket_acl;

/// All PUT requests pass through this function.
pub fn commands<P: AwsCredentialsProvider, D: DispatchSignedRequest>(matches: &ArgMatches, client: &mut Client<P,D>) -> Result<(), S3Error> {
    //println!("Bucket-put -- put::commands::{:#?}", matches);
    let bucket = matches.value_of("name").unwrap_or("");

    match matches.subcommand() {
        /// acl command.
        ("acl", Some(sub_matches)) => {
            // Default to Private
            let mut acl: CannedAcl = CannedAcl::Private;

            match sub_matches.subcommand() {
                ("public-read", _) => acl = CannedAcl::PublicRead,
                ("public-rw", _) => acl = CannedAcl::PublicReadWrite,
                ("public-readwrite", _) => acl = CannedAcl::PublicReadWrite,
                ("private", _) => acl = CannedAcl::Private,
                (e,_) => println!("Something {:?}", e),
            }

            let mut bucket_acl = PutBucketAclRequest::default();
            bucket_acl.bucket = bucket.to_string();

            // get acl option...
            bucket_acl.acl = Some(acl);

            match put_bucket_acl(bucket, &bucket_acl, client) {
                Ok(_) => {
                    let acl = get_bucket_acl(bucket, client);
                    if let Ok(acl) = acl {
                        print_acl_output(&acl, &client.output);
                    }
                },
                Err(_) => {},
            }
        },
        (e,_) => {
            if e.is_empty() && bucket.is_empty() {

                println!("what?");

            } else {
                let error = format!("incorrect or missing request {}", e);
                println_color!(term::color::RED, "{}", error);
            }
        }
    }

    Ok(())
}

fn put_bucket_acl<P: AwsCredentialsProvider, D: DispatchSignedRequest>(bucket: &str, acl: &PutBucketAclRequest, client: &Client<P,D>) -> Result<(), S3Error> {
    match client.s3client.put_bucket_acl(&acl) {
        Ok(val) => {
            Ok(val)
        },
        Err(e) => {
            let format = format!("{:#?}", e);
            print_error(&client.error, &format);
            Err(e)
        }
    }
}

fn print_acl_output(acl: &AccessControlPolicy, output: &Output) -> Result<(), S3Error> {
    println!("{:#?}", acl);

    Ok(())
}
