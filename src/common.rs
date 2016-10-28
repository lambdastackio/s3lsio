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


pub fn bucket(cli_bucket: String) -> Option<String> {
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
