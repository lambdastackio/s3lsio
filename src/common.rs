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

#![allow(unused_mut)]
#![allow(unused_assignments)]

use clap::ArgMatches;

/// Finds the bucket, object and last values based on the level of the ArgMatches
///
pub fn find_bucket_object_last<'a>(matches: &'a ArgMatches<'a>) -> (&'a str, String, &'a str) {
    let mut bucket: &str = "";
    let mut object: String = "".to_string();
    let mut last: &str = "";

    match matches.value_of("bucket") {
        Some(buck) => {
            let mut tmp_bucket: &str = "";

            if buck.contains("s3://") {
                tmp_bucket = &buck[5..];
            } else {
                tmp_bucket = buck.clone();
            }
            if tmp_bucket.contains('/') {
                let components: Vec<&str> = tmp_bucket.split('/').collect();
                let mut first: bool = true;
                let mut object_first: bool = true;

                for part in components {
                    if first {
                        bucket = part;
                    } else {
                        if !object_first {
                            object += "/";
                        }
                        object_first = false;
                        object += part;
                        last = part;
                    }
                    first = false;
                }
            } else {
                match tmp_bucket.trim() {
                    "." | "*" | "$" | "s3://" => {},
                    a @ _ => { bucket = a; },
                }
            }
        },
        None => {},
    }

    (bucket, object, last)
}

/// Simply returns the bucket from a bucket/object path
///
pub fn get_bucket(cli_bucket: String) -> Option<String> {
    let mut bucket: &str = "";

    if cli_bucket.contains('/') {
        let components: Vec<&str> = cli_bucket.split('/').collect();
        let mut first: bool = true;

        for part in components {
            if first {
                bucket = part;
                break;
            }
        }
    } else {
        bucket = cli_bucket.trim();
    }

    Some(bucket.to_string())
}
