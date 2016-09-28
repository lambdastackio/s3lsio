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

// NOTE: This attribute only needs to be set once.
#![doc(html_logo_url = "https://lambdastackio.github.io/static/images/lambdastack-200x200.png",
       html_favicon_url = "https://lambdastackio.github.io/static/images/favicon.ico",
       html_root_url = "https://lambdastackio.github.io/s3lsio/s3lsio/index.html")]

#[macro_use]
extern crate lsio;
extern crate aws_sdk_rust;
extern crate rustc_serialize;
extern crate term;
extern crate url;
extern crate uuid;
#[macro_use]
extern crate log;
extern crate env_logger;
#[macro_use]
extern crate clap;
extern crate pbr;
extern crate toml;

use std::io::{self, Write};
use std::env;
use std::path::{Path, PathBuf};

use clap::Shell;
use url::Url;

use aws_sdk_rust::aws::errors::s3::S3Error;
use aws_sdk_rust::aws::s3::endpoint::*;
use aws_sdk_rust::aws::s3::s3client::S3Client;
use aws_sdk_rust::aws::common::region::Region;
use aws_sdk_rust::aws::common::credentials::{AwsCredentialsProvider, DefaultCredentialsProvider};
use aws_sdk_rust::aws::common::request::DispatchSignedRequest;

use common::progress::ProgressBar;
use lsio::config::ConfigFile;

mod bucket;
mod object;
mod util;
mod common;
mod cli;
mod config;

/// Allows you to set the output type for stderr and stdout.
///
#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    JSON,
    PrettyJSON,
    Plain,
    Serialize,
    None,
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
          D: DispatchSignedRequest, // T: Write,
{
    pub s3client: &'a mut S3Client<P, D>,
    pub error: Error,
    pub output: Output,
    pub is_quiet: bool,
    pub is_default_config: bool, // pub pbr: ProgressBar<T>,
}

fn main() {
    // Gets overridden by cli option
    let mut is_quiet: bool = false;

    env_logger::init().unwrap();

    let version = format!("v{}", crate_version!());
    let mut home: PathBuf;
    // Get $HOME directory and set the default config. Let the CLI override the default.
    match env::home_dir() {
        Some(path) => {
            home = path;
            home.push(".s3lsio/config");
        },
        None => home = PathBuf::new(),
    }

    // NOTE: If the CLI info passed in does not meet the requirements then build_cli will panic!
    let matches = cli::build_cli("s3lsio", home.to_str().unwrap_or(""), &version).get_matches();

    if matches.is_present("generate-bash-completions") {
        cli::build_cli("s3lsio", home.to_str().unwrap_or(""), &version)
            .gen_completions_to("s3lsio", Shell::Bash, &mut io::stdout());
        ::std::process::exit(0);
    }

    // If the -q or --quiet flag was passed then shut off all output
    if matches.is_present("quiet") {
        is_quiet = true;
    }

    // NOTE: Get parameters or config for region, signature etc
    // Safe to unwrap since a default value is passed in. If a panic occurs then the environment
    // does not support a home directory.
    let config_option = matches.value_of("config").unwrap();
    let region = match matches.value_of("region").unwrap().to_string().to_lowercase().as_ref() {
        "useast1" => Region::UsEast1,
        "uswest1" => Region::UsWest1,
        "uswest2" => Region::UsWest2,
        "cnnorth1" => Region::CnNorth1,
        "eucentral1" => Region::EuCentral1,
        "euwest1" => Region::EuWest1,
        "saeast1" => Region::SaEast1,
        "apnortheast1" => Region::ApNortheast1,
        "apnortheast2" => Region::ApNortheast2,
        "apsouth1" => Region::ApSouth1,
        "apsoutheast1" => Region::ApSoutheast1,
        "apsoutheast2" => Region::ApSoutheast2,
        _ => Region::UsEast1,
    };

    // Option so None will be return if nothing is passed in.
    let ep_str = matches.value_of("endpoint");
    let proxy_str = matches.value_of("proxy");
    let signature_str = matches.value_of("signature");

    let output = match matches.value_of("output").unwrap().to_string().to_lowercase().as_ref() {
        "json" => OutputFormat::JSON,
        "pretty-json" => OutputFormat::PrettyJSON,
        "plain" => OutputFormat::Plain,
        "serialize" => OutputFormat::Serialize,
        _ => OutputFormat::PrettyJSON,
    };
    let output_color = match matches.value_of("output-color").unwrap().to_string().to_lowercase().as_ref() {
        "green" => term::color::GREEN,
        "red" => term::color::RED,
        "blue" => term::color::BLUE,
        "yellow" => term::color::YELLOW,
        "white" => term::color::WHITE,
        _ => term::color::GREEN,
    };

    // Set the config_file path to the default if a value is empty or set it to the passed in path value
    let mut config_file: PathBuf;
    if config_option.is_empty() {
        config_file = home.clone();
    } else {
        config_file = PathBuf::new();
        config_file.push(config_option);
    }

    let mut config = config::Config::from_file(config_file).unwrap_or(config::Config::default());

    // Let CLI args override any config setting if they exists.
    if ep_str.is_some() {
      config.set_endpoint(Some(Url::parse(ep_str.unwrap()).unwrap()));
    }

    if proxy_str.is_some() {
      config.set_proxy(Some(Url::parse(proxy_str.unwrap()).unwrap()));
    }

    if signature_str.is_some() {
      config.set_signature(signature_str.unwrap().to_string());
    } else {
      config.set_signature("V4".to_string());
    }
    let sign: String = config.signature.to_lowercase();

    let provider = DefaultCredentialsProvider::new(None).unwrap();

    let endpoint = Endpoint::new(region,
                                 if sign == "v2" {
                                     Signature::V2
                                 } else {
                                     Signature::V4
                                 },
                                 config.endpoint,
                                 config.proxy,
                                 Some(format!("s3lsio - {}", version)));

    let mut s3client = S3Client::new(provider, endpoint);

    let mut client = Client {
        s3client: &mut s3client,
        error: Error {
            format: OutputFormat::Serialize,
            color: term::color::RED,
        },
        output: Output {
            format: output,
            color: output_color,
        },
        is_quiet: is_quiet,
        is_default_config: config_option == home.to_str().unwrap(),
    };

    // Check which subcomamnd the user ran...
    let res = match matches.subcommand() {
        ("bucket", Some(sub_matches)) => bucket::commands(sub_matches, &mut client),
        ("object", Some(sub_matches)) => object::commands(sub_matches, &mut client),
        (e, _) => {
            println_color_quiet!(client.is_quiet, term::color::RED, "{}", e);
            Err(S3Error::new("A valid instruction is required"))
        },
    };

    if let Err(e) = res {
        println_color_quiet!(client.is_quiet, term::color::RED, "An error occured: {}", e);
        println_color_quiet!(client.is_quiet, term::color::RED, "{}", matches.usage());
        ::std::process::exit(1);
    }
}
