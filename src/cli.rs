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
        .author("Chris Jones <chris.jones@lambdastack.io>")
        .version(version)
        //.setting(AppSettings::SubcommandRequired)
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
                   .help("Specifies the output. Default is pretty-json. Options are json, pretty-json, plain and serialize")
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
       .subcommand(SubCommand::with_name("bucket")
            .about("Perform all bucket specific operations")
            .subcommand(SubCommand::with_name("delete")
                .about("Delete operation for bucket")
                .arg_from_usage("[name] 'Bucket name'"))
            .subcommand(SubCommand::with_name("get")
                .about("Get operation for bucket")
                .arg_from_usage("[name] 'Bucket name. Leave empty if getting list of buckets'")
                .subcommand(SubCommand::with_name("acl")
                    .about("Returns the bucket ACLs"))
                .subcommand(SubCommand::with_name("list")
                    .about("Returns the list of buckets")))
            .subcommand(SubCommand::with_name("put")
                .about("Put operation for bucket")
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
                .about("Delete operation for objects")
                .arg_from_usage("[name] 'Object name'"))
            .subcommand(SubCommand::with_name("get")
                .arg_from_usage("[bucket] 'Bucket name.'")
                .arg_from_usage("[name] 'Object name.'")
                .subcommand(SubCommand::with_name("acl"))
                    .about("Returns the object ACLs"))
            .subcommand(SubCommand::with_name("put")
                .about("Put operation for objects")
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
}
