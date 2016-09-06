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

extern crate num_cpus;

use clap::ArgMatches;
use aws_sdk_rust::aws::errors::s3::S3Error;

use term;
use Error;
use Output;
use OutputFormat;

// For Utilities...

pub fn print_error(format: &Error, out: &str) {
    let ref error = *format;
    match error.format {
        OutputFormat::Serialize => {
            println_color!(error.color, "{}", out);
        },
        OutputFormat::None => {},
        _ => println!("error"),
    }
}

pub fn print_output(format: &Output, out: &str) {
    let ref output = *format;
    match output.format {
        OutputFormat::Plain => {
            // Could have already been serialized before being passed to this function.
            println_color!(output.color, "{}", out);
        },
        OutputFormat::None => {},
        _ => println!("error"),
    }
}
