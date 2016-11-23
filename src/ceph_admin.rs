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
#![allow(unused_must_use)]
#![allow(unused_variables)]
#![allow(unused_assignments)]

use std::io;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::fs::File;
use std::ffi::OsStr;
use std::ops::Index;

use md5;
use term;
use rustc_serialize::json;
use rustc_serialize::base64::{STANDARD, ToBase64};
use clap::ArgMatches;
use rand::{thread_rng, Rng};

use aws_sdk_rust::aws::errors::s3::S3Error;
use aws_sdk_rust::aws::common::credentials::AwsCredentialsProvider;
use aws_sdk_rust::aws::common::request::DispatchSignedRequest;
use aws_sdk_rust::aws::common::common::Operation;
use aws_sdk_rust::aws::common::params::*;
use aws_sdk_rust::aws::s3::acl::*;
use aws_sdk_rust::aws::s3::bucket::*;
use aws_sdk_rust::aws::s3::object::*;
use aws_sdk_rust::aws::s3::admin::*;

// Use this for signing the admin feature for Ceph RGW
use aws_sdk_rust::aws::common::signature::*;

use Client;
use Output;
use OutputFormat;
use Commands;

static ALPHA_NUMERIC_LOWER: &'static str = "0123456789abcdefghijklmnopqrstuvwxyz";
static ALPHA_NUMERIC_UPPER: &'static str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
static ALPHA_NUMERIC: &'static str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// Holds the keys that were generated with the `admin key gen` command
#[derive(Debug, Default, Clone, RustcDecodable, RustcEncodable)]
pub struct AdminKeys {
    pub access_key: String,
    pub secret_key: String,
}


// CEPH RGW ONLY SECTION
/// Ceph Admin command function. Ability to perform everything radosgw-admin cli does
///
pub fn admin<P, D>(matches: &ArgMatches,
                   bucket: &str,
                   object: String,
                   client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    match matches.subcommand() {
        ("bucket", Some(matches)) => {
            let list = try!(buckets(matches, bucket, client));
            Ok(())
        },
        ("cap", Some(matches)) => {
            let list = try!(caps(matches, bucket, client));
            Ok(())
        },
        ("keys", Some(matches)) => {
            let list = try!(keys(matches, bucket, client));
            Ok(())
        },
        ("object", Some(matches)) => {
            let list = try!(objects(matches, bucket, &object, client));
            Ok(())
        },
        ("quota", Some(matches)) => {
            let list = try!(quota(matches, bucket, client));
            Ok(())
        },
        ("user", Some(matches)) => {
            let list = try!(user(matches, bucket, client));
            Ok(())
        },
        ("usage", Some(matches)) => {
            let list = try!(usage(matches, bucket, client));
            Ok(())
        },
        (e,_) => {
            let error = format!("Admin command {} not recognized", e);
            println_color_quiet!(client.is_quiet, term::color::RED, "{}", error);
            return Err(S3Error::new(error));
        },
    }
}

fn buckets<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut method = String::from("GET");

    let mut command = matches.value_of("command").unwrap_or("");
    let mut user = matches.value_of("user").unwrap_or("").to_string();
    let stats = matches.value_of("stats").unwrap_or("false").to_string().to_lowercase();

    // Make into macro...
    match user.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { user = "".to_string(); },
        a @ _ => { user = a.to_string(); },
    }
    match command.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { command = ""; },
        a @ _ => { command = a; },
    }

    let mut path: String = "admin/".to_string();
    let mut params = Params::new();
    let mut error: String = "".to_string();
    let mut path_options: Option<String> = None;

    match command {
        "delete" => {
            if bucket.is_empty() {
                error += &format!("Bucket value must be valid for delete command. ");
            }
            if !error.is_empty() {
                let e = S3Error::new(error);
                println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
                return Err(e);
            }
            path += "bucket";
            method = "DELETE".to_string();

            params.put("bucket", bucket);
            // NB: Could add an additional option but for now just remove the objects first.
            params.put("purge-objects", "true");
        },
        "index" => {
            if bucket.is_empty() {
                error += &format!("Bucket value must be valid for index command. ");
            }
            if !error.is_empty() {
                let e = S3Error::new(error);
                println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
                return Err(e);
            }
            let fix = matches.value_of("fix").unwrap_or("false").to_string().to_lowercase();
            let check = matches.value_of("check").unwrap_or("false").to_string().to_lowercase();

            path += "bucket";
            path_options = Some("?index&".to_string());
            params.put("bucket", bucket);
            if !fix.is_empty() {
                params.put("fix", &fix);
            }
            if !check.is_empty() {
                params.put("check-objects", &check);
            }
        },
        "link" => {
            if bucket.is_empty() {
                error += &format!("Bucket value must be valid for link command. ");
            }
            if user.is_empty() {
                error += &format!("User value must be valid for link command. ");
            }
            if !error.is_empty() {
                let e = S3Error::new(error);
                println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
                return Err(e);
            }
            method = "PUT".to_string();

            path += "bucket";
            params.put("bucket", bucket);
            params.put("uid", &user);
        },
        "ls" => {
            path += "metadata/bucket";
            if !user.is_empty() {
                params.put("uid", &user);
            }
        },
        "policy" => {
            path += "bucket";
            path_options = Some("?policy&".to_string());
            if !bucket.is_empty() {
                params.put("bucket", bucket);
            } else {
                let e = S3Error::new("Bucket must be valid");
                println_color_quiet!(client.is_quiet, client.error.color, "{}", e);
                return Err(e);
            }
        },
        "stats" => {
            path += "bucket";
            if !bucket.is_empty() {
                params.put("bucket", bucket);
            }
            params.put("stats", &stats);
            if !user.is_empty() {
                params.put("uid", &user);
            }
        },
        "unlink" => {
            if bucket.is_empty() {
                error += &format!("Bucket value must be valid for link command. ");
            }
            if user.is_empty() {
                error += &format!("User value must be valid for link command. ");
            }
            if !error.is_empty() {
                let e = S3Error::new(error);
                println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
                return Err(e);
            }
            method = "POST".to_string();

            path += "bucket";
            params.put("bucket", bucket);
            params.put("uid", &user);
        },
        _ => {
            path += "bucket";
        }
    }

    let mut request = AdminRequest::default();
    request.bucket = Some(bucket.to_string());
    request.method = Some(method);
    request.admin_path = Some(path);
    if path_options.is_some() {
        request.path_options = path_options;
    }
    request.params = params;
    if !user.is_empty() {
        request.uid = Some(user);
    }

    match client.s3client.admin(&request) {
        Ok(output) => {
            match client.output.format {
                OutputFormat::Serialize => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output.payload);
                },
                OutputFormat::Plain => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                },
                OutputFormat::JSON => {
                    if output.format == AdminOutputType::Json {
                        println_color_quiet!(client.is_quiet,
                                             client.output.color,
                                             "{}",
                                             output.payload);
                    }
                    else {
                        println_color_quiet!(client.is_quiet,
                                             client.output.color,
                                             "{}",
                                             json::encode(&output.payload).unwrap_or("{}".to_string()));
                    }
                },
                OutputFormat::PrettyJSON => {
                    if output.format == AdminOutputType::Json {
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                    }
                    else {
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output.payload));
                    }
                },
                _ => {},
            }
        },
        Err(e) => {
            let format = format!("{:#?}", e);
            let error = S3Error::new(format);
            println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", error);
            return Err(error);
        },
    }

    Ok(())
}

fn objects<P, D>(matches: &ArgMatches, bucket: &str, object: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let method = String::from("DELETE");

    let mut command = matches.value_of("command").unwrap_or("");
    match command.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { command = ""; },
        a @ _ => { command = a; },
    }

    let path: String = "admin/bucket".to_string();
    let mut params = Params::new();
    let mut error: String = "".to_string();

    match command {
        "delete" => {
            if bucket.is_empty() {
                error += &format!("Bucket value must be valid for delete command. ");
            }
            if object.is_empty() {
                error += &format!("Object value must be valid for delete command. ");
            }
            if !error.is_empty() {
                let e = S3Error::new(error);
                println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
                return Err(e);
            }
            params.put("bucket", bucket);
            params.put("object", object);
        },
        a @ _ => {
            let e = S3Error::new(format!("Invalid object command: {}", a));
            println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
            return Err(e);
        }
    }

    let mut request = AdminRequest::default();
    request.bucket = Some(bucket.to_string());
    request.method = Some(method);
    request.admin_path = Some(path);
    request.path_options = Some("?object&".to_string());
    request.params = params;

    match client.s3client.admin(&request) {
        Ok(output) => {
            match client.output.format {
                OutputFormat::Serialize => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output.payload);
                },
                OutputFormat::Plain => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                },
                OutputFormat::JSON => {
                    if output.format == AdminOutputType::Json {
                        println_color_quiet!(client.is_quiet,
                                             client.output.color,
                                             "{}",
                                             output.payload);
                    }
                    else {
                        println_color_quiet!(client.is_quiet,
                                             client.output.color,
                                             "{}",
                                             json::encode(&output.payload).unwrap_or("{}".to_string()));
                    }
                },
                OutputFormat::PrettyJSON => {
                    if output.format == AdminOutputType::Json {
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                    }
                    else {
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output.payload));
                    }
                },
                _ => {},
            }
        },
        Err(error) => {
            let format = format!("{:#?}", error);
            let error = S3Error::new(format);
            println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", error);
            return Err(error);
        },
    }

    Ok(())
}

fn quota<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut params = Params::new();
    let mut method = String::new();

    let mut user = matches.value_of("user").unwrap_or("").to_string();
    // Make into macro...
    match user.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { user = "".to_string(); },
        a @ _ => { user = a.to_string(); },
    }
    if user.is_empty() {
        let error = S3Error::new("User was not specified");
        println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", error);
        return Err(error);
    }

    params.put("uid", &user);

    let mut command = matches.value_of("command").unwrap_or("");
    match command.clone().trim() {
        "user" => {
            command = "user";
            params.put("quota-type", "user");
        },
        _ => {
            command = "bucket";
            params.put("quota-type", "bucket");
        },
    }
    let action = matches.value_of("action").unwrap_or("get").to_string().to_lowercase();
    match action.clone().as_ref() {
        "set" => { method = "PUT".to_string(); },
        "enable" => {
            method = "PUT".to_string();
            params.put("enabled", "true");
        },
        "disable" => {
            method = "PUT".to_string();
            params.put("enabled", "false");
        },
        _ => { method = "GET".to_string(); },
    }

    if action.clone() == "set".to_string() {
        let mut size_str = matches.value_of("size").unwrap_or("").to_string();
        //match &size_str.clone().to_lowercase() as &str {
        match size_str.clone().to_lowercase().as_ref() {
            "" | "." | "*" | "$" | "s3://" => {},
            a @ _ => {
                size_str = a.to_string();
                if size_str == "0".to_string() {
                    size_str = "-1".to_string();
                }
                params.put("max-size-kb", &size_str);
            },
        }

        let mut object_str = matches.value_of("count").unwrap_or("").to_string();
        //match &object_str.clone().to_lowercase() as &str {
        match object_str.clone().to_lowercase().as_ref() {
            "" | "." | "*" | "$" | "s3://" => {},
            a @ _ => {
                object_str = a.to_string();
                if object_str == "0".to_string() {
                    object_str = "-1".to_string();
                }
                params.put("max-objects", &object_str);
            },
        }
    }

    let path: String = "admin/user".to_string();

    let mut request = AdminRequest::default();
    request.bucket = Some(bucket.to_string());
    request.method = Some(method);
    request.admin_path = Some(path);
    request.path_options = Some("?quota&".to_string());
    request.params = params;

    match client.s3client.admin(&request) {
        Ok(output) => {
            match client.output.format {
                OutputFormat::Serialize => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output.payload);
                },
                OutputFormat::Plain => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                },
                OutputFormat::JSON => {
                    if output.format == AdminOutputType::Json {
                        println_color_quiet!(client.is_quiet,
                                             client.output.color,
                                             "{}",
                                             output.payload);
                    }
                    else {
                        println_color_quiet!(client.is_quiet,
                                             client.output.color,
                                             "{}",
                                             json::encode(&output.payload).unwrap_or("{}".to_string()));
                    }
                },
                OutputFormat::PrettyJSON => {
                    if output.format == AdminOutputType::Json {
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                    }
                    else {
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output.payload));
                    }
                },
                _ => {},
            }
        },
        Err(error) => {
            let format = format!("{:#?}", error);
            let error = S3Error::new(format);
            println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", error);
            return Err(error);
        },
    }

    Ok(())
}

fn caps<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut method = String::from("PUT");
    let path: String = "admin/user".to_string();
    let params: Params;

    let sub_params = match matches.subcommand() {
        ("create", Some(sub_matches)) => {
            user_caps(sub_matches, bucket, &client)
        },
        ("delete", Some(sub_matches)) => {
            method = "DELETE".to_string();
            user_caps(sub_matches, bucket, &client)
        },
        (_, _) => { Err(S3Error::new("Unrecognized command")) },
    };

    match sub_params {
        Ok(subparams) => params = subparams.unwrap(),
        Err(e) => {
            let error = S3Error::new(format!("{}", e));
            println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
            return Err(error);
        },
    }

    let mut request = AdminRequest::default();
    request.bucket = Some(bucket.to_string());
    request.method = Some(method);
    request.admin_path = Some(path);
    request.path_options = Some("?caps&".to_string());
    request.params = params;

    match client.s3client.admin(&request) {
        Ok(output) => {
            match client.output.format {
                OutputFormat::Serialize => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output.payload);
                },
                OutputFormat::Plain => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                },
                OutputFormat::JSON => {
                    if output.format == AdminOutputType::Json {
                        println_color_quiet!(client.is_quiet,
                                             client.output.color,
                                             "{}",
                                             output.payload);
                    }
                    else {
                        println_color_quiet!(client.is_quiet,
                                             client.output.color,
                                             "{}",
                                             json::encode(&output.payload).unwrap_or("{}".to_string()));
                    }
                },
                OutputFormat::PrettyJSON => {
                    if output.format == AdminOutputType::Json {
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                    }
                    else {
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output.payload));
                    }
                },
                _ => {},
            }
        },
        Err(error) => {
            let format = format!("{:#?}", error);
            let error = S3Error::new(format);
            println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
            return Err(error);
        },
    }

    Ok(())
}

fn user_caps<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<Option<Params>, S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut params = Params::new();

    let mut user = matches.value_of("user").unwrap_or("");
    match user.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { return Err(S3Error::new("User was not specified")); },
        a @ _ => {
            user = a;
            params.put("uid", user);
        },
    }

    let mut caps = matches.value_of("caps").unwrap_or("");
    match caps.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { return Err(S3Error::new("Caps was not specified")); },
        a @ _ => {
            caps = a;
            params.put("user-caps", caps);
        },
    }

    Ok(Some(params))
}

fn user<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut method = String::from("GET");
    let mut path: String = "admin/user".to_string();
    let params: Params;

    let sub_params = match matches.subcommand() {
        ("create", Some(sub_matches)) => {
            method = "PUT".to_string();
            user_create(sub_matches, bucket, &client)
        },
        ("delete", Some(sub_matches)) => {
            method = "DELETE".to_string();
            user_delete(sub_matches, bucket, &client)
        },
        ("ls", Some(sub_matches)) => {
            path = "admin/metadata/user".to_string();
            user_get_list(sub_matches, bucket, false, &client)
        },
        ("modify", Some(sub_matches)) => {
            method = "POST".to_string();
            user_modify(sub_matches, bucket, &client)
        },
        (_, Some(sub_matches)) => user_get_list(sub_matches, bucket, true, &client),
        (_, None) => { Err(S3Error::new("Unrecognized command")) },
    };

    match sub_params {
        Ok(subparams) => params = subparams.unwrap(),
        Err(e) => {
            let error = S3Error::new(format!("{}", e));
            println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
            return Err(error);
        },
    }

    let mut request = AdminRequest::default();
    request.bucket = Some(bucket.to_string());
    request.method = Some(method);
    request.admin_path = Some(path);
    request.params = params;

    match client.s3client.admin(&request) {
        Ok(output) => {
            match client.output.format {
                OutputFormat::Serialize => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output.payload);
                },
                OutputFormat::Plain => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                },
                OutputFormat::JSON => {
                    if output.format == AdminOutputType::Json {
                        println_color_quiet!(client.is_quiet,
                                             client.output.color,
                                             "{}",
                                             output.payload);
                    }
                    else {
                        println_color_quiet!(client.is_quiet,
                                             client.output.color,
                                             "{}",
                                             json::encode(&output.payload).unwrap_or("{}".to_string()));
                    }
                },
                OutputFormat::PrettyJSON => {
                    if output.format == AdminOutputType::Json {
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                    }
                    else {
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output.payload));
                    }
                },
                _ => {},
            }
        },
        Err(error) => {
            let format = format!("{:#?}", error);
            let error = S3Error::new(format);
            println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
            return Err(error);
        },
    }

    Ok(())
}

fn user_create<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<Option<Params>, S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut params = Params::new();

    let mut user = matches.value_of("user").unwrap_or("");
    match user.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { return Err(S3Error::new("User was not specified")); },
        a @ _ => {
            user = a;
            params.put("uid", user);
        },
    }

    let mut display_name = matches.value_of("display_name").unwrap_or("");
    match display_name.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { return Err(S3Error::new("Display-name was not specified")); },
        a @ _ => {
            display_name = a;
            params.put("display-name", display_name);
        },
    }

    let mut email = matches.value_of("email").unwrap_or("");
    match email.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { email = ""; },
        a @ _ => {
            email = a;
            params.put("email", email);
        },
    }

    let mut access_key = matches.value_of("access_key").unwrap_or("");
    match access_key.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { access_key = ""; },
        a @ _ => {
            access_key = a;
            params.put("access-key", access_key);
        },
    }

    let mut secret_key = matches.value_of("secret_key").unwrap_or("");
    match secret_key.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { secret_key = ""; },
        a @ _ => {
            secret_key = a;
            params.put("secret-key", secret_key);
        },
    }

    let mut suspended = matches.value_of("suspended").unwrap_or("false");
    match suspended.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { suspended = "false"; },
        a @ _ => {
            suspended = a;
            params.put("suspended", suspended);
        },
    }

    let mut caps = matches.value_of("caps").unwrap_or("");
    match caps.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { caps = ""; },
        a @ _ => {
            caps = a;
            params.put("caps", caps);
        },
    }

    Ok(Some(params))
}

fn user_delete<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<Option<Params>, S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut user = matches.value_of("user").unwrap_or("");
    match user.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { user = ""; },
        a @ _ => { user = a; },
    }

    if user.is_empty() {
        return Err(S3Error::new("User was not specified"));
    }

    let mut purge_data = matches.value_of("purge_data").unwrap_or("true");
    match purge_data.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { purge_data = "true"; },
        a @ _ => { purge_data = a; },
    }

    let mut params = Params::new();
    params.put("uid", user);
    params.put("purge-data", purge_data);

    Ok(Some(params))
}

fn user_get_list<P, D>(matches: &ArgMatches, bucket: &str, user_required: bool, client: &Client<P, D>) -> Result<Option<Params>, S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut user = matches.value_of("user").unwrap_or("");
    match user.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { user = ""; },
        a @ _ => { user = a; },
    }

    if user.is_empty() && user_required {
        return Err(S3Error::new("User was not specified"));
    }

    let mut params = Params::new();
    params.put("uid", user);

    Ok(Some(params))
}

fn user_modify<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<Option<Params>, S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut params = Params::new();

    let mut user = matches.value_of("user").unwrap_or("");
    match user.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { return Err(S3Error::new("User was not specified")); },
        a @ _ => {
            user = a;
            params.put("uid", user);
        },
    }

    let mut display_name = matches.value_of("display_name").unwrap_or("");
    match display_name.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { display_name = ""; },
        a @ _ => {
            display_name = a;
            params.put("display-name", display_name);
        },
    }

    let mut suspended = matches.value_of("suspended").unwrap_or("false");
    match suspended.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { suspended = "false"; },
        a @ _ => {
            suspended = a;
            params.put("suspended", suspended);
        },
    }

    let mut email = matches.value_of("email").unwrap_or("");
    match email.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { email = ""; },
        a @ _ => {
            email = a;
            params.put("email", email);
        },
    }

    let mut access_key = matches.value_of("access_key").unwrap_or("");
    match access_key.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { access_key = ""; },
        a @ _ => {
            access_key = a;
            params.put("access-key", access_key);
        },
    }

    let mut secret_key = matches.value_of("secret_key").unwrap_or("");
    match secret_key.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { secret_key = ""; },
        a @ _ => {
            secret_key = a;
            params.put("secret-key", secret_key);
        },
    }

    let mut caps = matches.value_of("caps").unwrap_or("");
    match caps.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { caps = ""; },
        a @ _ => {
            caps = a;
            params.put("caps", caps);
        },
    }

    let mut max = matches.value_of("max_buckets").unwrap_or("");
    match max.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { max = ""; },
        a @ _ => {
            max = a;
            params.put("max-buckets", max);
        },
    }

    Ok(Some(params))
}

fn usage<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut method = String::from("GET");
    let path: String = "admin/usage".to_string();
    let params: Params;

    let sub_params = match matches.subcommand() {
        ("trim", Some(sub_matches)) => {
            method = "DELETE".to_string();
            usage_trim(sub_matches, bucket, &client)
        },
        (_, Some(sub_matches)) => usage_list(sub_matches, bucket, &client),
        (_, None) => { Err(S3Error::new("Unrecognized command")) },
    };

    match sub_params {
        Ok(subparams) => params = subparams.unwrap(),
        Err(e) => {
            let error = S3Error::new(format!("{}", e));
            println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
            return Err(error);
        },
    }

    let mut request = AdminRequest::default();
    request.bucket = Some(bucket.to_string());
    request.method = Some(method);
    request.admin_path = Some(path);
    request.params = params;

    match client.s3client.admin(&request) {
        Ok(output) => {
            match client.output.format {
                OutputFormat::Serialize => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output.payload);
                },
                OutputFormat::Plain => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                },
                OutputFormat::JSON => {
                    if output.format == AdminOutputType::Json {
                        println_color_quiet!(client.is_quiet,
                                             client.output.color,
                                             "{}",
                                             output.payload);
                    }
                    else {
                        println_color_quiet!(client.is_quiet,
                                             client.output.color,
                                             "{}",
                                             json::encode(&output.payload).unwrap_or("{}".to_string()));
                    }
                },
                OutputFormat::PrettyJSON => {
                    if output.format == AdminOutputType::Json {
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                    }
                    else {
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output.payload));
                    }
                },
                _ => {},
            }
        },
        Err(error) => {
            let format = format!("{:#?}", error);
            let error = S3Error::new(format);
            println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", error);
            return Err(error);
        },
    }

    Ok(())
}

fn usage_list<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<Option<Params>, S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut params = Params::new();

    let mut user = matches.value_of("user").unwrap_or("");
    match user.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => {
            user = "";
        },
        a @ _ => {
            user = a;
            params.put("uid", user);
        },
    }

    let mut start = matches.value_of("start").unwrap_or("");
    match start.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { start = ""; },
        a @ _ => {
            start = a;
            params.put("start", start);
        },
    }

    let mut end = matches.value_of("end").unwrap_or("");
    match end.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { end = ""; },
        a @ _ => {
            end = a;
            params.put("end", end);
        },
    }

    let mut show_entries = matches.value_of("show_entries").unwrap_or("false");
    match show_entries.clone().trim() {
        "true" => {
            show_entries = "true";
            params.put("show-entries", "true");
        },
        _ => {
            show_entries = "false";
        },
    }

    let mut show_summary = matches.value_of("show_summary").unwrap_or("false");
    match show_summary.clone().trim() {
        "true" => {
            show_summary = "true";
            params.put("show-summary", "true");
        },
        _ => {
            show_summary = "false";
        },
    }

    Ok(Some(params))
}

fn usage_trim<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<Option<Params>, S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut params = Params::new();

    let mut user = matches.value_of("user").unwrap_or("");
    match user.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => {
            user = "";
        },
        a @ _ => {
            user = a;
            params.put("uid", user);
        },
    }

    let mut start = matches.value_of("start").unwrap_or("");
    match start.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { start = ""; },
        a @ _ => {
            start = a;
            params.put("start", start);
        },
    }

    let mut end = matches.value_of("end").unwrap_or("");
    match end.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { end = ""; },
        a @ _ => {
            end = a;
            params.put("end", end);
        },
    }

    let mut remove_all = matches.value_of("remove_all").unwrap_or("true");
    match remove_all.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => {
            remove_all = "true";
        },
        a @ _ => {
            remove_all = a;
        },
    }

    if !user.is_empty() {
        remove_all = "true";
    }
    params.put("remove-all", remove_all);

    Ok(Some(params))
}

fn keys<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut method = String::from("GET");
    let path: String = "admin/user".to_string();
    let params: Params;

    let sub_params = match matches.subcommand() {
        ("create", Some(sub_matches)) => {
            method = "PUT".to_string();
            user_key_create(sub_matches, bucket, &client)
        },
        ("delete", Some(sub_matches)) => {
            method = "DELETE".to_string();
            user_key_delete(sub_matches, bucket, &client)
        },
        ("gen", Some(sub_matches)) => {
            return key_generate(sub_matches, &client);
        },
        (_, _) => { Err(S3Error::new("Unrecognized command")) },
    };

    match sub_params {
        Ok(subparams) => params = subparams.unwrap(),
        Err(e) => {
            let error = S3Error::new(format!("{}", e));
            println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
            return Err(error);
        },
    }

    let mut request = AdminRequest::default();
    request.bucket = Some(bucket.to_string());
    request.method = Some(method);
    request.admin_path = Some(path);
    request.path_options = Some("?key&".to_string());
    request.params = params;

    match client.s3client.admin(&request) {
        Ok(output) => {
            match client.output.format {
                OutputFormat::Serialize => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output.payload);
                },
                OutputFormat::Plain => {
                    // Could have already been serialized before being passed to this function.
                    println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                },
                OutputFormat::JSON => {
                    if output.format == AdminOutputType::Json {
                        println_color_quiet!(client.is_quiet,
                                             client.output.color,
                                             "{}",
                                             output.payload);
                    }
                    else {
                        println_color_quiet!(client.is_quiet,
                                             client.output.color,
                                             "{}",
                                             json::encode(&output.payload).unwrap_or("{}".to_string()));
                    }
                },
                OutputFormat::PrettyJSON => {
                    if output.format == AdminOutputType::Json {
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", output.payload);
                    }
                    else {
                        println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output.payload));
                    }
                },
                _ => {},
            }
        },
        Err(error) => {
            let format = format!("{:#?}", error);
            let error = S3Error::new(format);
            println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
            return Err(error);
        },
    }

    Ok(())
}

fn key_generate<P, D>(matches: &ArgMatches, client: &Client<P, D>) -> Result<(), S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let keys = AdminKeys::default();
    let mut access_key: String = String::new();
    let mut secret_key: String = String::new();
    let alpha_numeric_upper_len: u8 = ALPHA_NUMERIC_UPPER.len() as u8;
    let alpha_numeric_len: u8 = ALPHA_NUMERIC.len() as u8;
    let alpha_numeric_upper_vec = ALPHA_NUMERIC_UPPER.to_string().into_bytes();
    let alpha_numeric_vec = ALPHA_NUMERIC.to_string().into_bytes();
    let ak_len = 20;
    let sk_len = 40;

    // access_key max length 20
    // secret_key max length 40

    let mut ak_buf = [0u8; 20];
    let mut sk_buf = [0u8; 40];

    thread_rng().fill_bytes(&mut ak_buf);
    thread_rng().fill_bytes(&mut sk_buf);

    for i in 0..ak_len {
        let pos: u8 = ak_buf[i];
        let chr = alpha_numeric_upper_vec[(pos % alpha_numeric_upper_len) as usize];
        access_key.push(chr as char);
    }

    for i in 0..sk_len {
        let pos: u8 = sk_buf[i];
        let chr = alpha_numeric_vec[(pos % alpha_numeric_len) as usize];
        secret_key.push(chr as char);
    }

    println!("{:?}", access_key);
    println!("{:?}", secret_key);

/*
    match client.output.format {
        OutputFormat::JSON | OutputFormat::PrettyJSON => {
            println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&keys));
        },
        _ => {
            // Plain
            println_color_quiet!(client.is_quiet, client.output.color, "{}", "output.payload");
        },
    }
*/
    Ok(())
}

fn user_key_create<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<Option<Params>, S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut params = Params::new();

    let mut user = matches.value_of("user").unwrap_or("");
    match user.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { return Err(S3Error::new("User was not specified")); },
        a @ _ => {
            user = a;
            params.put("uid", user);
        },
    }

    let mut generate_key = matches.value_of("generate_key").unwrap_or("");
    match generate_key.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { generate_key = "true"; },
        a @ _ => {
            generate_key = a;
            params.put("generate-key", generate_key);
        },
    }

    let mut access_key = matches.value_of("access_key").unwrap_or("");
    match access_key.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { access_key = "" },
        a @ _ => {
            access_key = a;
            params.put("access-key", access_key);
        },
    }

    let mut secret_key = matches.value_of("secret_key").unwrap_or("");
    match secret_key.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { secret_key = ""; },
        a @ _ => {
            secret_key = a;
            params.put("secret-key", secret_key);
        },
    }

    Ok(Some(params))
}

fn user_key_delete<P, D>(matches: &ArgMatches, bucket: &str, client: &Client<P, D>) -> Result<Option<Params>, S3Error>
    where P: AwsCredentialsProvider,
          D: DispatchSignedRequest,
{
    let mut params = Params::new();

    let mut access_key = matches.value_of("access_key").unwrap_or("");
    match access_key.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { return Err(S3Error::new("Access-key was not specified")); },
        a @ _ => {
            access_key = a;
            params.put("access-key", access_key);
        },
    }

    let mut user = matches.value_of("user").unwrap_or("");
    match user.clone().trim() {
        "" | "." | "*" | "$" | "s3://" => { user = ""; },
        a @ _ => {
            user = a;
            params.put("uid", user);
        },
    }

    Ok(Some(params))
}
