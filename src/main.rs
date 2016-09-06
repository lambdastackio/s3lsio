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

// NOTE: This attribute only needs to be set once.
#![doc(html_logo_url = "https://www.rust-lang.org/logos/rust-logo-128x128-blk-v2.png",
       html_favicon_url = "https://www.rust-lang.org/favicon.ico",
       html_root_url = "https://lambdastackio.github.io/s3lsio/s3lsio/index.html")]

#[macro_use]
extern crate lsio;
extern crate aws_sdk_rust;
extern crate term;
extern crate url;
extern crate uuid;
#[macro_use]
extern crate log;
extern crate env_logger;
#[macro_use]
extern crate clap;
extern crate unicase;

use std::io::{self, Write};

use clap::{App, SubCommand, AppSettings};

use aws_sdk_rust::aws::errors::s3::S3Error;
use aws_sdk_rust::aws::s3::endpoint::*;
use aws_sdk_rust::aws::s3::s3client::S3Client;
use aws_sdk_rust::aws::common::region::Region;
use aws_sdk_rust::aws::common::credentials::{DefaultCredentialsProvider, AwsCredentialsProvider};
use aws_sdk_rust::aws::common::request::DispatchSignedRequest;

/// Allows you to set the output type for stderr and stdout.
///
#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    json,
    plain,
    serialize,
    none,
}

// Error and Output can't have derive(debug) because term does not have some of it's structs
// using fmt::debug etc.

/// Allows you to control Error output.
///
pub struct Error {
    /// Defaults to OutputFormat::serialize since it's easier to debug.
    ///
    /// Available formats are json, plain, serialize or none (don't output anything)
    pub format: OutputFormat,
    /// Can be any term color. Defaults to term::color::RED.
    pub color: term::color::Color,
}

/// Allows you to control non-Error output.
///
pub struct Output {
    /// Defaults to OutputFormat::plain.
    ///
    /// Available formats are json, plain, serialize or none (don't output anything).
    /// If plain is used then you can serialize structures with format! and then pass the output.
    pub format: OutputFormat,
    /// Can be any term color. Defaults to term::color::GREEN.
    pub color: term::color::Color,
}

/// Client structure holds a reference to the S3Client which also implements two traits:
/// AwsCredentialsProvider and DispatchSignedRequest
/// Since S3Client struct is takes those two traits as parameters then ALL functions called
/// that require passing in S3Client or Client must specify the trait signature as follows:
/// Example: fn whatever_function<P: AwsCredentialsProvider, D: DispatchSignedRequest>(client: &mut Client<P,D>)
/// Note: Could also specify 'where' P:... D:... instead.
///
pub struct Client<'a, P: 'a, D: 'a>
        where P: AwsCredentialsProvider,
              D: DispatchSignedRequest,
{
    pub s3client: &'a mut S3Client<P, D>,
    pub error: Error,
    pub output: Output,
}

mod bucket;
mod object;
mod util;
mod common;

const NAME: &'static str = "s3lsio";

fn main() {
    env_logger::init().unwrap();

    let matches = App::new(NAME)
                    .about("S3 Client Utility that can access AWS S3, Ceph or any third party S3 enable environment")
                    .author("Chris Jones <chris.jones@lambdastack.io>")
                    // Get the version from our Cargo.toml using clap's crate_version!() macro
                    .version(&*format!("v{}", crate_version!()))
                    .setting(AppSettings::SubcommandRequired)
                    .after_help("For more information about a specific command, try `s3lsio <command> --help`\nSource code for s3lsio available at: https://github.com/lambdastackio/s3lsio")
                    .subcommand(SubCommand::with_name("signature")
                        .about("Overrides the default API signature of V4"))
                    .subcommand(SubCommand::with_name("proxy")
                        .about("Allows for proxy url with port"))
                    .subcommand(SubCommand::with_name("bucket")
                        .about("Perform all bucket specific operations")
                        .subcommand(SubCommand::with_name("delete")
                            .arg_from_usage("[name] 'Bucket name'"))
                        .subcommand(SubCommand::with_name("get")
                            .arg_from_usage("[name] 'Bucket name. Leave empty if getting list of buckets'")
                            .subcommand(SubCommand::with_name("acl"))
                                .about("Returns the bucket ACLs"))
                        .subcommand(SubCommand::with_name("put")
                            .arg_from_usage("[name] 'Bucket name'")
                            .subcommand(SubCommand::with_name("acl")
                                .about("Sets the bucket ACLs")
                                .subcommand(SubCommand::with_name("public-read")
                                    .about("Allows the public to read the bucket content"))
                                .subcommand(SubCommand::with_name("public-readwrite")
                                    .about("Allows the public to read/write the bucket content"))
                                .subcommand(SubCommand::with_name("public-rw")
                                    .about("Allows the public to read/write the bucket content"))
                                .subcommand(SubCommand::with_name("private")
                                    .about("Sets the bucket content to private")))))
                    .subcommand(SubCommand::with_name("object")
                        .about("Perform all object specific operations")
                        .subcommand(SubCommand::with_name("delete")
                            .arg_from_usage("[name] 'Object name'"))
                        .subcommand(SubCommand::with_name("get")
                            .arg_from_usage("[name] 'Object name. Leave empty if getting list of objects'")
                            .subcommand(SubCommand::with_name("acl"))
                                .about("Returns the object ACLs"))
                        .subcommand(SubCommand::with_name("put")
                            .arg_from_usage("[name] 'Object name'")
                            .subcommand(SubCommand::with_name("acl")
                                .about("Sets the Object ACLs")
                                .subcommand(SubCommand::with_name("public-read")
                                    .about("Allows the public to read the object"))
                                .subcommand(SubCommand::with_name("public-readwrite")
                                    .about("Allows the public to read/write the object"))
                                .subcommand(SubCommand::with_name("public-rw")
                                    .about("Allows the public to read/write the object"))
                                .subcommand(SubCommand::with_name("private")
                                    .about("Sets the object to private")))))
                    .subcommand(SubCommand::with_name("endpoint")
                        .about("Specify the S3 endpoint")
                        .arg_from_usage("[url] '(Optional) - Endpoint for the S3 interface'"))
                    .get_matches();

    // NOTE: Get parameters or config for region, signature etc
    let provider = DefaultCredentialsProvider::new(None).unwrap();

    let endpoint = Endpoint::new(
                            Region::UsEast1,
                            Signature::V2,
                            None,
                            None,
                            None);

    let mut s3client = S3Client::new(
                            provider,
                            endpoint);

    let mut client = Client{s3client: &mut s3client,
                            error: Error{format: OutputFormat::serialize,
                                         color: term::color::RED},
                            output: Output{format: OutputFormat::plain,
                                         color: term::color::GREEN}};

    // Check which subcomamnd the user ran...
    let res = match matches.subcommand() {
        ("bucket", Some(sub_matches)) => bucket::commands(sub_matches, &mut client),
        ("object", Some(sub_matches)) => object::commands(sub_matches, &mut client),
        (e, _) => {
            println!("{}", e);
            Err(S3Error::new("incorrect request"))
        },
    };

    if let Err(e) = res {
        writeln!(&mut io::stderr(), "An error occured:\n{}", e).ok();
        ::std::process::exit(1);
    }
}
