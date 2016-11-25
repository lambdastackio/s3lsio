Name:    s3lsio
Version: 0.1.18
Release: 0
Summary: AWS S3 and Ceph command-line utility and benchmarking.

License: Apache2
Source0: s3lsio

%description
S3lsio is a command-line utility to access AWS S3 and Ceph. Also includes benchmarking and Ceph RGW Admin features.

%install
mkdir -p $RPM_BUILD_ROOT/usr/bin
install -p -m 0755 %{SOURCE0} $RPM_BUILD_ROOT/usr/bin

%files
/usr/bin/%{name}
