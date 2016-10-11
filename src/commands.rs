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

use std::io;
use std::io::{Read, Seek, SeekFrom, BufReader, Write};
use std::path::Path;
use std::fs::File;
use std::ffi::OsStr;

use term;
use rustc_serialize::json;

use clap::ArgMatches;
use aws_sdk_rust::aws::errors::s3::S3Error;
use aws_sdk_rust::aws::common::credentials::AwsCredentialsProvider;
use aws_sdk_rust::aws::common::request::DispatchSignedRequest;
use aws_sdk_rust::aws::s3::acl::*;
use aws_sdk_rust::aws::s3::bucket::*;
use aws_sdk_rust::aws::s3::object::*;

use Client;
use Output;
use OutputFormat;
use Commands;

/// Commands
pub fn commands<P, D>(matches: &ArgMatches,
                      cmd: Commands,
                      client: &mut Client<P,D>)
                      -> Result<(), S3Error>
                      where P: AwsCredentialsProvider,
                            D: DispatchSignedRequest {
  let mut bucket: &str = "";
  let mut object: String = "".to_string();
  let mut last: &str = "";
  // Make sure the s3 schema prefix is present
  let (scheme, tmp_bucket) = matches.value_of("bucket").unwrap_or("s3:// ").split_at(5);

  if tmp_bucket.contains("/") {
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
    bucket = tmp_bucket.trim();
    object = "".to_string();
  }

  match cmd {
    Commands::get => {
      let mut path = matches.value_of("path").unwrap_or("").to_string();
      if path.is_empty() {
        path = last.to_string();
      }
      let result = get_object(bucket, &object, &path, client);
      Ok(())
    },
    Commands::put => {
      let path = matches.value_of("path").unwrap_or("");
      let result = put_object(bucket, &object, path, client);
      Ok(())
    },
    Commands::rm => {
      let version = matches.value_of("version").unwrap_or("");
      let result = delete_object(bucket, &object, version, client);
      Ok(())
    },
    Commands::acl => {
      if object.is_empty() {
        let acl = try!(get_bucket_acl(bucket, client));
        match client.output.format {
            OutputFormat::Plain => {
                // Could have already been serialized before being passed to this function.
                println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", acl);
            },
            OutputFormat::JSON => {
                println_color_quiet!(client.is_quiet, client.output.color, "{}", json::encode(&acl).unwrap_or("{}".to_string()));
            },
            OutputFormat::PrettyJSON => {
                println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&acl));
            },
            OutputFormat::None => {},
            OutputFormat::NoneAll => {},
            e @ _ => println_color_quiet!(client.is_quiet, client.error.color, "Error: Format - {:#?}", e),
        }
      } else {
        let acl = try!(get_object_acl(bucket, &object, client));
      }
      Ok(())
    },
    Commands::head => {
      if object.is_empty() {
        let list = try!(get_bucket_head(bucket, client));
      } else {
        let list = try!(get_object_head(bucket, &object, client));
      }
      Ok(())
    },
    Commands::ls => {
      let version = matches.value_of("versions").unwrap_or("");
      if bucket.is_empty() {
        let list = try!(get_buckets_list(client));
      } else {
        if bucket.contains("/") {
          let components: Vec<&str> = bucket.split('/').collect();
          bucket = components[0];
          if components[1].len() == 0 {
            // List objects in bucket.
            if version.is_empty() {
              let list = try!(get_object_list(bucket, client));
            } else {
              let list = try!(get_object_version_list(bucket, version, client));
            }
          }
        }
        else {
          if version.is_empty() {
            let list = try!(get_object_list(bucket, client));
          } else {
            let list = try!(get_object_version_list(bucket, version, client));
          }
        }
      }
      Ok(())
    },
    /// create new bucket
    Commands::mb => {
      if bucket.is_empty() {
        let error = format!("missing bucket name");
        println_color_quiet!(client.is_quiet, term::color::RED, "{}", error);
        Err(S3Error::new(error))
      } else {
        let result = create_bucket(bucket, client);
        Ok(())
      }
    },
    Commands::rb => {
      let result = delete_bucket(bucket, client);
      Ok(())
    },
    Commands::setacl => {
      let result = try!(set_bucket_acl(matches, bucket, client));
      Ok(())
    },
    Commands::setver => {
      let list = try!(set_bucket_versioning(matches, bucket, client));
      Ok(())
    },
    Commands::ver => {
      let list = try!(get_bucket_versioning(bucket, client));
      Ok(())
    },
  };

  Ok(())
}

fn create_bucket<P, D>(bucket: &str,
                           client: &Client<P, D>)
                           -> Result<(), S3Error>
                           where P: AwsCredentialsProvider,
                                 D: DispatchSignedRequest {
    let mut request = CreateBucketRequest::default();
    request.bucket = bucket.to_string();

    match client.s3client.create_bucket(&request) {
        Ok(_) => {
          if (client.output.format != OutputFormat::None) ||
             (client.output.format != OutputFormat::NoneAll) {
              println_color_quiet!(client.is_quiet, client.output.color, "Success");
          }
          Ok(())
        },
        Err(e) => {
          println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
          Err(e)
        },
    }
}

fn delete_bucket<P, D>(bucket: &str,
                       client: &Client<P, D>)
                       -> Result<(), S3Error>
                       where P: AwsCredentialsProvider,
                             D: DispatchSignedRequest {
    let request = DeleteBucketRequest { bucket: bucket.to_string() };

    match client.s3client.delete_bucket(&request) {
        Ok(_) => {
          if (client.output.format != OutputFormat::None) ||
             (client.output.format != OutputFormat::NoneAll) {
              println_color_quiet!(client.is_quiet, client.output.color, "Success");
          }
          Ok(())
        },
        Err(e) => {
          println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
          Err(e)
        },
    }
}

// Get functions...
fn get_bucket_head<P, D>(bucket: &str,
                         client: &Client<P,D>)
                         -> Result<(), S3Error>
                         where P: AwsCredentialsProvider,
                               D: DispatchSignedRequest {
     let request = HeadBucketRequest { bucket: bucket.to_string() };

     match client.s3client.head_bucket(&request) {
         Ok(_) => {
           if (client.output.format != OutputFormat::None) ||
              (client.output.format != OutputFormat::NoneAll) {
              // May want to put in json format later??
              println_color_quiet!(client.is_quiet, client.output.color, "Bucket exists");
           }
           Ok(())
         },
         Err(e) => {
           println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
           Err(e)
         },
     }
}

fn get_bucket_versioning<P, D>(bucket: &str,
                               client: &Client<P,D>)
                               -> Result<(), S3Error>
                               where P: AwsCredentialsProvider,
                                     D: DispatchSignedRequest {
     let request = GetBucketVersioningRequest { bucket: bucket.to_string() };

     match client.s3client.get_bucket_versioning(&request) {
         Ok(output) => {
           match client.output.format {
             OutputFormat::Serialize => {
                 // Could have already been serialized before being passed to this function.
                 println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
             },
             OutputFormat::Plain => {
                 // Could have already been serialized before being passed to this function.
                 println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
             },
             OutputFormat::JSON => {
                 println_color_quiet!(client.is_quiet, client.output.color, "{}", json::encode(&output).unwrap_or("{}".to_string()));
             },
             OutputFormat::PrettyJSON => {
                 println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output));
             },
             OutputFormat::Simple => {
               println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output));
             },
             OutputFormat::None => {},
             OutputFormat::NoneAll => {},
           }
           Ok(())
         },
         Err(e) => {
           println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
           Err(e)
         },
     }
}

fn get_bucket_acl<P, D>(bucket: &str,
                        client: &Client<P,D>)
                        -> Result<AccessControlPolicy, S3Error>
                        where P: AwsCredentialsProvider,
                              D: DispatchSignedRequest {
    let mut request = GetBucketAclRequest::default();
    request.bucket = bucket.to_string();

    match client.s3client.get_bucket_acl(&request) {
        Ok(acl) => Ok(acl),
        Err(e) => {
            println_color_quiet!(client.is_quiet, client.error.color, "{:?}", e);
            Err(e)
        }
    }
}

fn get_buckets_list<P, D>(client: &Client<P,D>)
                          -> Result<(), S3Error>
                          where P: AwsCredentialsProvider,
                                D: DispatchSignedRequest {
    match client.s3client.list_buckets() {
      Ok(output) => {
          match client.output.format {
              OutputFormat::Serialize => {
                  // Could have already been serialized before being passed to this function.
                  println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
              },
              OutputFormat::Plain => {
                  // Could have already been serialized before being passed to this function.
                  println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
              },
              OutputFormat::JSON => {
                  println_color_quiet!(client.is_quiet, client.output.color, "{}", json::encode(&output).unwrap_or("{}".to_string()));
              },
              OutputFormat::PrettyJSON => {
                  println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output));
              },
              OutputFormat::Simple => {
                for bucket in output.buckets {
                  println_color_quiet!(client.is_quiet, client.output.color, "s3://{}/", bucket.name);
                }
              },
              OutputFormat::None => {},
              OutputFormat::NoneAll => {},
          }
          Ok(())
      }
      Err(error) => {
          let format = format!("{:#?}", error);
          let error = S3Error::new(format);
          println_color_quiet!(client.is_quiet, client.error.color, "{:?}", error);
          Err(error)
      }
    }
}

// Set functions...
fn set_bucket_acl<P, D>(matches: &ArgMatches,
                            bucket: &str,
                            client: &Client<P, D>)
                            -> Result<(), S3Error>
                            where P: AwsCredentialsProvider,
                                  D: DispatchSignedRequest {

    let acl: CannedAcl;
    let cli_acl = matches.value_of("acl").unwrap_or("").to_string().to_lowercase();

    match cli_acl.as_ref() {
      "public-read" => acl = CannedAcl::PublicRead,
      "public-rw" => acl = CannedAcl::PublicReadWrite,
      "public-readwrite" => acl = CannedAcl::PublicReadWrite,
      "private" => acl = CannedAcl::Private,
      _ => {
        println_color_quiet!(client.is_quiet, client.error.color, "missing acl: public-read, public-rw, public-readwrite or private");
        return Err(S3Error::new("missing acl: public-read, public-rw, public-readwrite or private"));
      },
    }

    let mut request = PutBucketAclRequest::default();
    request.bucket = bucket.to_string();

    // get acl option...
    request.acl = Some(acl);

    match client.s3client.put_bucket_acl(&request) {
      Ok(output) => {
        let acl = get_bucket_acl(bucket, client);
        if let Ok(acl) = acl {
            match client.output.format {
              OutputFormat::Serialize => {
                  // Could have already been serialized before being passed to this function.
                  println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", acl);
              },
              OutputFormat::Plain => {
                  // Could have already been serialized before being passed to this function.
                  println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", acl);
              },
              OutputFormat::JSON => {
                  println_color_quiet!(client.is_quiet, client.output.color, "{}", json::encode(&acl).unwrap_or("{}".to_string()));
              },
              OutputFormat::PrettyJSON => {
                  println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&acl));
              },
              OutputFormat::Simple => {
                println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&acl));
              },
              OutputFormat::None => {},
              OutputFormat::NoneAll => {},
            }
        }
      },
      Err(e) => {
        let error = format!("{:#?}", e);
        println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
        return Err(S3Error::new(error));
      },
    }

    Ok(())
}

fn set_bucket_versioning<P, D>(matches: &ArgMatches,
                               bucket: &str,
                               client: &Client<P, D>)
                               -> Result<(), S3Error>
                               where P: AwsCredentialsProvider,
                                     D: DispatchSignedRequest {
    let cli_ver = matches.value_of("ver").unwrap_or("").to_string().to_lowercase();

    let request = PutBucketVersioningRequest{
        bucket: bucket.to_string(),
        versioning_configuration: VersioningConfiguration {
            status: if cli_ver == "on" {"Enabled".to_string()} else {"Suspended".to_string()},
            mfa_delete: "".to_string(),
        },
        mfa: None,
        content_md5: None,
    };

    match client.s3client.put_bucket_versioning(&request) {
        Ok(()) => {
          if (client.output.format != OutputFormat::None) ||
             (client.output.format != OutputFormat::NoneAll) {
            println_color_quiet!(client.is_quiet, client.output.color, "Success");
          }
          Ok(())
        },
        Err(e) => {
          println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
          Err(e)
        },
    }
}


// Objects...
fn get_object_list<P, D>(bucket: &str,
                         client: &Client<P,D>)
                         -> Result<(), S3Error>
                         where P: AwsCredentialsProvider,
                               D: DispatchSignedRequest {
    let mut request = ListObjectsRequest::default();
    request.bucket = bucket.to_string();
    request.version = Some(2);

    match client.s3client.list_objects(&request) {
      Ok(output) => {
        match client.output.format {
          OutputFormat::Serialize => {
              // Could have already been serialized before being passed to this function.
              println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
          },
          OutputFormat::Plain => {
              // Could have already been serialized before being passed to this function.
              println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
          },
          OutputFormat::JSON => {
              println_color_quiet!(client.is_quiet, client.output.color, "{}", json::encode(&output).unwrap_or("{}".to_string()));
          },
          OutputFormat::PrettyJSON => {
              println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output));
          },
          OutputFormat::Simple => {
            for object in output.contents {
              println_color_quiet!(client.is_quiet, client.output.color, "s3://{}/{}", bucket, object.key);
            }
          },
          OutputFormat::None => {},
          OutputFormat::NoneAll => {},
        }

        Ok(())
      }
      Err(error) => {
          let format = format!("{:#?}", error);
          let error = S3Error::new(format);
          println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", error);
          Err(error)
      }
    }
}

fn get_object_version_list<P, D>(bucket: &str,
                                 version: &str,
                                 client: &Client<P,D>)
                                 -> Result<(), S3Error>
                                 where P: AwsCredentialsProvider,
                                       D: DispatchSignedRequest {
    let mut request = ListObjectVersionsRequest::default();
    request.bucket = bucket.to_string();

    match client.s3client.list_object_versions(&request) {
      Ok(output) => {
        match client.output.format {
          OutputFormat::Serialize => {
              // Could have already been serialized before being passed to this function.
              println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
          },
          OutputFormat::Plain => {
              // Could have already been serialized before being passed to this function.
              println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
          },
          OutputFormat::JSON => {
              println_color_quiet!(client.is_quiet, client.output.color, "{}", json::encode(&output).unwrap_or("{}".to_string()));
          },
          OutputFormat::PrettyJSON => {
              println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output));
          },
          OutputFormat::Simple => {
            //for object in output.contents {
              //println_color_quiet!(client.is_quiet, client.output.color, "s3://{}/{}", bucket, object.key);
            //}
          },
          OutputFormat::None => {},
          OutputFormat::NoneAll => {},
        }

        Ok(())
      }
      Err(error) => {
          let format = format!("{:#?}", error);
          let error = S3Error::new(format);
          println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", error);
          Err(error)
      }
    }
}

// Limited in file size.
fn get_object<P, D>(bucket: &str,
                    object: &str,
                    path: &str,
                    client: &Client<P,D>)
                    -> Result<(), S3Error>
                    where P: AwsCredentialsProvider,
                          D: DispatchSignedRequest {
   let mut request = GetObjectRequest::default();
   request.bucket = bucket.to_string();
   request.key = object.to_string();

   match client.s3client.get_object(&request) {
     Ok(output) => {
       // NoneAll means no writing to disk or stdout
       if client.output.format != OutputFormat::NoneAll {
         let mut file = File::create(path).unwrap();
         match file.write_all(&output.body) {
           Ok(_) => {
             // NOTE: Need to remove body from output (after it writes out) by making it mut so that
             // items below can output metadata OR place body in different element than others.
             match client.output.format {
               OutputFormat::Serialize => {
                 println_color_quiet!(client.is_quiet, client.output.color, "Success");
               },
               OutputFormat::Plain => {
                   println_color_quiet!(client.is_quiet, client.output.color, "Success");
               },
               OutputFormat::JSON => {
                 println_color_quiet!(client.is_quiet, client.output.color, "Success");
               },
               OutputFormat::PrettyJSON => {
                 println_color_quiet!(client.is_quiet, client.output.color, "Success");
               },
               OutputFormat::Simple => {
                 println_color_quiet!(client.is_quiet, client.output.color, "Success");
               },
               OutputFormat::None => {},
               OutputFormat::NoneAll => {},
             }
             Ok(())
           },
           Err(e) => {
             let error = format!("{:#?}", e);
             println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
             Err(S3Error::new(error))
           }
         }
      } else {
        Ok(())
      }
     },
     Err(e) => {
       println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
       Err(e)
     },
   }
}

fn get_object_head<P, D>(bucket: &str,
                         object: &str,
                         client: &Client<P,D>)
                         -> Result<(), S3Error>
                         where P: AwsCredentialsProvider,
                               D: DispatchSignedRequest {
   let mut request = HeadObjectRequest::default();
   request.bucket = bucket.to_string();
   request.key = object.to_string();

   match client.s3client.head_object(&request) {
     Ok(output) => {
       match client.output.format {
         OutputFormat::Serialize => {
           println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
         },
         OutputFormat::Plain => {
           println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
         },
         OutputFormat::JSON => {
           println_color_quiet!(client.is_quiet, client.output.color, "{}", json::encode(&output).unwrap_or("{}".to_string()));
         },
         OutputFormat::PrettyJSON => {
           println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output));
         },
         OutputFormat::Simple => {
           println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
         },
         OutputFormat::None => {},
         OutputFormat::NoneAll => {},
       }
       Ok(())
     },
     Err(e) => {
       println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e);
       Err(e)
     },
   }
}

fn get_object_acl<P, D>(bucket: &str,
                        object: &str,
                        client: &Client<P, D>)
                        -> Result<(), S3Error>
                        where P: AwsCredentialsProvider,
                              D: DispatchSignedRequest {
  let mut request = GetObjectAclRequest::default();
  request.bucket = bucket.to_string();
  request.key = object.to_string();

  match client.s3client.get_object_acl(&request) {
    Ok(output) => {
      match client.output.format {
        OutputFormat::Serialize => {
          println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
        },
        OutputFormat::Plain => {
          println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
        },
        OutputFormat::JSON => {
          println_color_quiet!(client.is_quiet, client.output.color, "{}", json::encode(&output).unwrap_or("{}".to_string()));
        },
        OutputFormat::PrettyJSON => {
          println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output));
        },
        OutputFormat::Simple => {
          println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
        },
        OutputFormat::None => {},
        OutputFormat::NoneAll => {},
      }
      Ok(())
    },
    Err(e) => {
      let format = format!("{:#?}", e);
      println_color_quiet!(client.is_quiet, client.error.color, "{:?}", e);
      Err(e)
    }
  }
}

// Limited in file size. Max is 5GB but should use Multipart upload for larger than 15MB.
fn put_object<P, D>(bucket: &str,
                    key: &str,
                    object: &str,
                    client: &Client<P, D>)
                    -> Result<(), S3Error>
                    where P: AwsCredentialsProvider,
                          D: DispatchSignedRequest {
  let file = File::open(object).unwrap();
  let metadata = file.metadata().unwrap();

  let mut buffer: Vec<u8> = Vec::with_capacity(metadata.len() as usize);

  match file.take(metadata.len()).read_to_end(&mut buffer) {
      Ok(_) => {},
      Err(e) => {
        let error = format!("Error reading file {}", e);
        return Err(S3Error::new(error));
      },
  }

  let correct_key: String;
  if key.is_empty() {
    let path = Path::new(object);
    correct_key = path.file_name().unwrap().to_str().unwrap().to_string();
  } else {
    correct_key = key.to_string();
  }
  let mut request = PutObjectRequest::default();
  request.bucket = bucket.to_string();
  request.key = correct_key;
  request.body = Some(&buffer);

  match client.s3client.put_object(&request) {
      Ok(output) => {
        match client.output.format {
          OutputFormat::Serialize => {
            println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
          },
          OutputFormat::Plain => {
            println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
          },
          OutputFormat::JSON => {
            println_color_quiet!(client.is_quiet, client.output.color, "{}", json::encode(&output).unwrap_or("{}".to_string()));
          },
          OutputFormat::PrettyJSON => {
            println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output));
          },
          OutputFormat::Simple => {
            println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
          },
          OutputFormat::None => {},
          OutputFormat::NoneAll => {},
        }
        Ok(())
      },
      Err(e) => {
        let error = format!("{:#?}", e);

        println!("{:?}", request);

        println_color_quiet!(client.is_quiet, client.error.color, "{}", error);
        Err(S3Error::new(error))
      },
  }
}

fn delete_object<P, D>(bucket: &str,
                       object: &str,
                       version: &str,
                       client: &Client<P, D>)
                       -> Result<(), S3Error>
                       where P: AwsCredentialsProvider,
                             D: DispatchSignedRequest {
   let mut request = DeleteObjectRequest::default();
   request.bucket = bucket.to_string();
   request.key = object.to_string();
   if !version.is_empty() {
     request.version_id = Some(version.to_string());
   }

   match client.s3client.delete_object(&request) {
       Ok(output) => {
         match client.output.format {
           OutputFormat::Serialize => {
             println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
           },
           OutputFormat::Plain => {
             println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
           },
           OutputFormat::JSON => {
             println_color_quiet!(client.is_quiet, client.output.color, "{}", json::encode(&output).unwrap_or("{}".to_string()));
           },
           OutputFormat::PrettyJSON => {
             println_color_quiet!(client.is_quiet, client.output.color, "{}", json::as_pretty_json(&output));
           },
           OutputFormat::Simple => {
             println_color_quiet!(client.is_quiet, client.output.color, "{:#?}", output);
           },
           OutputFormat::None => {},
           OutputFormat::NoneAll => {},
         }
       },
       Err(e) => println_color_quiet!(client.is_quiet, client.error.color, "{:#?}", e),
   }

  Ok(())
}
