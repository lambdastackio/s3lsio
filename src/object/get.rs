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

pub fn commands<P: AwsCredentialsProvider, D: DispatchSignedRequest>(matches: &ArgMatches, client: &mut Client<P,D>) -> Result<(), S3Error> {
    //println!("Bucket-get -- get::commands::{:#?}", matches);
    let bucket = matches.value_of("bucket").unwrap_or("");
    let object = matches.value_of("name").unwrap_or("");

    match matches.subcommand() {
        /// acl command.
        ("acl", _) => {
            let acl = try!(get_object_acl(bucket, object, client));
            let output = print_acl_output(&acl, &client.output);
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

pub fn get_object_acl<P: AwsCredentialsProvider, D: DispatchSignedRequest>(bucket: &str, object: &str, client: &Client<P,D>) -> Result<AccessControlPolicy, S3Error> {
    let mut get_object_acl = GetObjectAclRequest::default();
    get_object_acl.bucket = bucket.to_string();
    get_object_acl.key = object.to_string();

    match client.s3client.get_object_acl(&get_object_acl) {
        Ok(acl) => Ok(acl),
        Err(e) => {
            let format = format!("{:#?}", e);
            println_color!(term::color::RED, "missing or incorrect bucket name or object name");
            print_error(&client.error, &format);
            Err(e)
        }
    }
}

fn print_acl_output(acl: &AccessControlPolicy, output: &Output) -> Result<(), S3Error> {
    println!("{:#?}", acl);

    Ok(())
}
