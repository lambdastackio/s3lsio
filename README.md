## S3lsio

![s3lsio](https://img.shields.io/crates/v/s3lsio.svg) [![Crates.io](https://img.shields.io/crates/d/s3lsio.svg)]() [![Crates.io](https://img.shields.io/crates/l/s3lsio.svg)]()

### Cloning and Committing PRs
git clone --recursive https://github.com/lambdastackio/s3lsio.git

Do the above command instead of the normal git clone ... since there is a submodule for the OSX package manager Homebrew.
However, if you don't care about that then you can simply leave --recursive off and git clone as normal.

### Installing
If you only want to install `s3lsio` then pick from your environment below:

`OSX (Homebrew)`

The `brew tap` command below is important to setup where Homebrew will look for the package and updates.

>brew tap lambdastackio/tap

>brew install s3lsio

`Linux RPMs` - (replace 0.1.18 with latest version)

>wget https://s3.amazonaws.com/s3lsio/osx/s3lsio-0.1.18.tar.gz

>tar -xzvf s3lsio-0.1.18.tar.gz

>sudo rpm -Uvh s3lsio-0.1.18.rpm

If you have `Rust` installed and want to use cargo
>cargo install s3lsio

To install Rust (if needed):
(Linux and Mac) curl -sSf https://static.rust-lang.org/rustup.sh | sh
(Windows) Here is the link to the official Rust downloads page: https://www.rust-lang.org/en-US/downloads.html

### About
AWS S3 command line utility written in rust. Works with both V2 and V4 signatures. This is important when working
with third party systems that implement an S3 interface like Ceph. Ceph Hammer and below use V2 while Jewel and higher
use V4.

### Design
Simple as possible but flexible enough to be used in cron jobs, every day utility use, scripts etc. Additional AWS type storage options are coming soon.

### Packaging
Deb/RPM - EVR (Epoch.Version.Release) - Follow semantic versioning which is now standard. Currently, you have to maintain the versioning information on packages, Cargo.toml and CLI which is a pain because you will often forget to properly update each of those. The CLI is now reading from the toml file but the others are not. So, an effort will soon be underway in a build.rs process to dynamically update those to match the Cargo.toml file before building begins to keep everything in sync.

OSX:
A git submodule exists to the homebrew-tap repo in lambdastackio. This will create an updated tarball that Homebrew uses to install and update packages.

Linux (RHEL/CentOS/Fedora):
A Vagrantfile exists to dynamically pull down a VirtualBox instance and spin up the correct OS to pull down the github code, install Rust, build the code, build the rpms and push them to S3.

Linux Ubuntu:
A Vagrantfile exists to dynamically pull down a VirtualBox instance and spin up the correct OS to pull down the github code, install Rust, build the code, build the debs and push them to S3.

The above process will most likely move to Docker soon.

NB. Once this process is fully baked it will be rolled out as a template process for all binary crates moving forward. It's possible that I may auto codegen a Pacman package file to be used in Windows MinGW-64bit.

### Changes
There are a lot of changes happening at a rapid rate. What may not be there today may very well be there tomorrow.
