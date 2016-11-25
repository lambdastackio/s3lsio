#!/bin/sh
#
# Copyright 2016 LambdaStack All rights reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
# http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# NOTE: Building a RHEL or CentOS version (assumes version dev tools installed)
# sudo yum install -y openssl openssl-devel cmake
# Get nightly version of rust because lint tools depend on it. Don't use "stable" version
# curl -sSf https://static.rust-lang.org/rustup.sh | sh -s -- --channel=nightly

# NOTE: This *ONLY* builds the s3lsio.spec file to be built before pushing to the repo.
# Once on the repo the complete repo can be pulled down to a RHEL based machine with Rust on it to actually buid the rpms.

REPO_BASE=$(git rev-parse --show-toplevel)

cargo build --release

VERSION=$($REPO_BASE/target/release/s3lsio --version | awk '{print $2}')

lsiotemplate -d "{\"version\": \"$VERSION\"}" -t s3lsio.spec.hbs -o specs/s3lsio.spec json
