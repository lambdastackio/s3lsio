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

use Client;

pub mod get;
pub mod put;
pub mod delete;

/// All bucket commands are passed through this function.
pub fn commands<P: AwsCredentialsProvider, D: DispatchSignedRequest>(matches: &ArgMatches, client: &mut Client<P,D>) -> Result<(), S3Error> {
    //println!("Bucket -- {:#?}", matches);

    match matches.subcommand() {
        ("get", Some(sub_matches)) => {
            match get::commands(sub_matches, client) {
                Err(e) => Err(e),
                Ok(_) => Ok(())
            }
        }
        ("put", Some(sub_matches)) => {
            match put::commands(sub_matches, client) {
                Err(e) => Err(e),
                Ok(_) => Ok(())
            }
        },
        ("delete", Some(sub_matches)) => delete::commands(sub_matches),
        (e, _) => {
            let error = format!("incorrect or missing request {}", e);
            println!("{}", error);
            Err(S3Error::new(error))
        },
    };

    Ok(())
}
