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

use clap::{App, Arg, SubCommand};

pub fn build_cli<'a>(app: &str, home: &'a str, version: &'a str) -> App<'a, 'a> {
  App::new(app)
    .about("S3 Client Utility that can access AWS S3, Ceph or any third party S3 enable environment")
    .author("Chris Jones")
    .version(version)
    .after_help("For more information about a specific command, try `s3lsio <command> --help`\nSource code for s3lsio available at: https://github.com/lambdastackio/s3lsio")
    .arg(Arg::with_name("generate-bash-completions")
      .short("g")
      .long("generate-bash-completions")
      .help("Outputs bash completions"))
    .arg(Arg::with_name("config")
      .short("c")
      .long("config")
      .value_name("FILE")
      .default_value(home)
      .help("Sets a custom config file. Default is $HOME/.s3lsio/config")
      .takes_value(true))
   .arg(Arg::with_name("endpoint")
      .short("e")
      .long("endpoint")
      .value_name("URL:<port>")
      .help("Sets a custom endpoint URL:<port> (port is optional). Default is AWS default endpoints based on Region")
      .takes_value(true))
   .arg(Arg::with_name("output-color")
      .short("l")
      .long("output-color")
      .default_value("green")
      .value_name("green or red or blue or yellow or white or normal")
      .help("Specifies the output color. Default is green.")
      .takes_value(true))
   .arg(Arg::with_name("output")
      .short("o")
      .long("output")
      .default_value("pretty-json")
      .value_name("pretty-json or json or plain or serialize")
      .help("Specifies the output to stdout (and disk in some cases). Default is pretty-json. Options are json, none, noneall, pretty-json, plain, serialize")
      .takes_value(true))
   .arg(Arg::with_name("proxy")
      .short("p")
      .long("proxy")
      .value_name("URL:<port>")
      .help("Sets a custom proxy URL:<port>. Default is to ready http(s)_proxy")
      .takes_value(true))
   .arg(Arg::with_name("quiet")
      .short("q")
      .long("quiet")
      .help("No output is produced"))
   .arg(Arg::with_name("region")
      .short("r")
      .long("region")
      .value_name("Region")
      .default_value("UsEast1")
      .help("Sets S3 Region. Default is UsEast1")
      .takes_value(true))
   .arg(Arg::with_name("signature")
      .short("s")
      .long("signature")
      .value_name("V2 or V4")
      .default_value("V4")
      .help("Sets an API Signature version. Default is V4")
      .takes_value(true))
   .arg(Arg::with_name("yes")
      .short("y")
      .long("yes")
      .help("Answer yes automatically"))
   .subcommand(SubCommand::with_name("abort")
      .about("Abort multipart upload: s3lsio abort <upload_id> s3://<bucket>/<object>")
      .arg_from_usage("[upload_id] 'Multipart Upload ID'")
      .arg_from_usage("[bucket] 'Bucket name'"))
   .subcommand(SubCommand::with_name("acl")
      .about("Bucket ACLs: s3lsio acl s3://<bucket>")
      .arg_from_usage("[bucket] 'Bucket name'"))
   .subcommand(SubCommand::with_name("head")
      .about("Head Bucket: s3lsio head s3://<bucket>")
      .arg_from_usage("[bucket] 'Bucket name'"))
   .subcommand(SubCommand::with_name("ls")
      .about("List Buckets or Objects in bucket with optional version tag: s3lsio ls OR s3lsio ls s3://<bucket>/<prefix> <option>")
      .arg_from_usage("[bucket] 'Bucket name'")
      .arg_from_usage("[option] 'ver or multi'")
      .arg_from_usage("[upload_id] 'multipart upload ID option'"))
   .subcommand(SubCommand::with_name("mb")
      .about("Make Bucket: s3lsio mb s3://<bucket>")
      .arg_from_usage("[bucket] 'Bucket name'"))
   .subcommand(SubCommand::with_name("rb")
      .about("Remove Bucket: s3lsio rb s3://<bucket>")
      .arg_from_usage("[bucket] 'Bucket name'"))
   .subcommand(SubCommand::with_name("rm")
      .about("Remove Object and/or Object version: s3lsio rm s3://<bucket>/<object> <version>")
      .arg_from_usage("[bucket] 'Bucket name'")
      .arg_from_usage("[version] 'Version'"))
   .subcommand(SubCommand::with_name("get")
      .about("Get Object: s3lsio get s3://<bucket>/<object> <path>")
      .arg_from_usage("[bucket] 'Bucket name'")
      .arg_from_usage("[path] 'Path'"))
   .subcommand(SubCommand::with_name("put")
      .about("Put Object <size of parts> is optional: s3lsio put <path> s3://<bucket>/<object> <size of parts>")
      .arg_from_usage("[path] 'Path'")
      .arg_from_usage("[bucket] 'Bucket name'")
      .arg_from_usage("[size] 'Size of parts'"))
   .subcommand(SubCommand::with_name("range")
      .about("Byte-Range request of Object: s3lsio range <offset> <len> s3://<bucket>/<object> <path>")
      .arg_from_usage("[offset] 'Range begin offset'")
      .arg_from_usage("[len] 'Range len'")
      .arg_from_usage("[bucket] 'Bucket name'")
      .arg_from_usage("[path] 'Path'"))
   .subcommand(SubCommand::with_name("setacl")
      .about("Bucket Versioning: s3lsio setacl <acl> s3://<bucket>")
      .arg_from_usage("[acl] 'ACL - public-read, public-readwrite, private'")
      .arg_from_usage("[bucket] 'Bucket name'"))
   .subcommand(SubCommand::with_name("setver")
      .about("Enables Bucket Versioning: s3lsio setver on|off s3://<bucket>")
      .arg_from_usage("[ver] 'On or Off'")
      .arg_from_usage("[bucket] 'Bucket name'"))
   .subcommand(SubCommand::with_name("ver")
      .about("Shows Bucket Versioning: s3lsio ver s3://<bucket>")
      .arg_from_usage("[bucket] 'Bucket name'"))
/*
   .subcommand(SubCommand::with_name("bucket")
      .about("Perform all bucket specific operations. Example: bucket command <your bucket name>...")
      .subcommand(SubCommand::with_name("create")
            .about("Create a bucket")
            .arg_from_usage("[name] 'Bucket name'"))
        .subcommand(SubCommand::with_name("delete")
            .about("Delete operation for bucket")
            .arg_from_usage("[name] 'Bucket name'"))
        .subcommand(SubCommand::with_name("get")
            .about("Get operation for bucket")
            .arg_from_usage("[name] 'Bucket name'")
            .subcommand(SubCommand::with_name("acl")
                .about("Returns the bucket ACLs"))
            .subcommand(SubCommand::with_name("head")
                .about("Returns the bucket head"))
            .subcommand(SubCommand::with_name("versioning")
                .about("Returns versioning")))
        .subcommand(SubCommand::with_name("list")
            .about("Lists the buckets"))
        .subcommand(SubCommand::with_name("put")
            .about("Put operation for bucket")
            .arg_from_usage("[name] 'Bucket name'")
            .subcommand(SubCommand::with_name("acl")
                .about("Sets the bucket ACLs: public-read, public-readwrite, public-rw, private")
                .subcommand(SubCommand::with_name("public-read")
                    .about("Allows the public to read the bucket content"))
                .subcommand(SubCommand::with_name("public-readwrite")
                    .about("Allows the public to read/write the bucket content"))
                .subcommand(SubCommand::with_name("public-rw")
                    .about("Allows the public to read/write the bucket content"))
                .subcommand(SubCommand::with_name("private")
                    .about("Sets the bucket content to private")))
            .subcommand(SubCommand::with_name("versioning")
                .about("Enables versioning for a bucket"))))
   .subcommand(SubCommand::with_name("object")
        .about("Perform all object specific operations. Example: object <your bucket name> command <your object name> <options>...")
        .arg_from_usage("[bucket] 'Bucket name'")
        .subcommand(SubCommand::with_name("delete")
            .about("DELETE operation for objects")
            .arg_from_usage("[object] 'Object name'")
            .arg_from_usage("[version] 'Object Version ID'"))
        .subcommand(SubCommand::with_name("get")
            .about("GET operations for objects")
            .arg_from_usage("[object] 'Object name'")
            .arg_from_usage("[path] 'Full file path'")
            .subcommand(SubCommand::with_name("acl")
                .about("Returns the object ACLs"))
            .subcommand(SubCommand::with_name("head")
                .about("Returns object metadata")))
        .subcommand(SubCommand::with_name("list")
            .about("Returns a list of objects for a bucket"))
        .subcommand(SubCommand::with_name("put")
            .about("PUT operations for objects. Example: object mybucket put /path/of/my/object/myobject")
            .arg_from_usage("[object] 'Object name'")
            .arg_from_usage("[path] 'Full file path'")
            .subcommand(SubCommand::with_name("acl")
                .about("Sets the Object ACLs")
                .subcommand(SubCommand::with_name("public-read")
                    .about("Allows the public to read the object"))
                .subcommand(SubCommand::with_name("public-readwrite")
                    .about("Allows the public to read/write the object"))
                .subcommand(SubCommand::with_name("public-rw")
                    .about("Allows the public to read/write the object"))
                .subcommand(SubCommand::with_name("private")
                    .about("Sets the object to private")))
            .subcommand(SubCommand::with_name("key")
                .about("Use as object key name")
                .arg_from_usage("[key] 'Object Key'"))))
*/
}
