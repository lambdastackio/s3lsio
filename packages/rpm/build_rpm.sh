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

# NOTE: MUST be ran as root

BASE=/root/rpmbuild
SPEC=s3lsio.spec

yum update -y
yum install -y rpm-build

mkdir -p $BASE/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
cp specs/$SPEC $BASE/SPECS/$SPEC

# Use -bb option instead of -ba since we're not building from source and do not need a source rpm produced.
rpmbuild -bb $BASE/SPECS/$SPEC

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
