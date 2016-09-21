## S3lsio
### Installing
If you have Rust installed then you can simply do the following and it will compile for your environment.
Install: cargo install s3lsio

At present, there is not a binary version for each environment so Rust is required. After general release there will be
packages for each platform supported.

To install Rust (if needed):
(Linux and Mac) curl -sSf https://static.rust-lang.org/rustup.sh | sh
(Windows) Here is the link to the official Rust downloads page: https://www.rust-lang.org/en-US/downloads.html

### About
AWS S3 command line utility written in rust. Works with both V2 and V4 signatures. This is important when working
with third party systems that implement an S3 interface like Ceph. Ceph Hammer and below use V2 while Jewel and higher
use V4.

### Design
Simple as possible but flexible enough to be used in cron jobs, every day utility use, scripts etc.

### Changes
There are a lot of changes happening at a rapid rate. What may not be there today may very well be there tomorrow.
