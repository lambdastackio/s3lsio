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

# NOTE: MUST be ran as root

REPO_BASE=$(git rev-parse --show-toplevel)
USER=$(whoami)

cargo build --release

# Has to be as root...
APP=s3lsio
ARCH=x86_64
BASE=/root/rpmbuild
SPEC=$APP.spec
RELEASE=target/release/$APP

sudo yum update -y
sudo yum install -y rpm-build

# Remove any older versions...
sudo rm -rf $BASE/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
# Create a new directory structure
sudo mkdir -p $BASE/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
sudo cp $REPO_BASE/packages/rpm/specs/$SPEC $BASE/SPECS/$SPEC

# Copy over base
sudo cp $REPO_BASE/$RELEASE $BASE/SOURCES

# Use -bb option instead of -ba since we're not building from source and do not need a source rpm produced.
sudo rpmbuild -bb $BASE/SPECS/$SPEC

sudo sh -c "cp $BASE/RPMS/$ARCH/*.rpm $REPO_BASE"
sudo chown $USER:$USER $REPO_BASE/*.rpm

# Install it to make sure all works
sudo rpm -Uvh $REPO_BASE/*.rpm

# If you need it in another architecture instead of x86_64 then change the spec file and add BuildArch: <whatever here>
# below Source0:
#
# 64bit RPM will be found in $BASE/RPMS/x86_64/*.rpm

# Install: (Note: the command-lines below assume in current directory of rpm to install. Also, root or sudo rights)
# rpm -Uvh s3lsio-<whatever version etc>.x86_64.rpm
# OR
# yum install -y s3lsio--<whatever version etc>.x86_64.rpm
#
# Remove:
# rpm -e --allmatches s3lsio
# yum remove -y s3lsio
#
# Check to make sure it's there:
# yum info s3lsio
