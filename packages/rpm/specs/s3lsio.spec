Name:    s3lsio
Version: 0.1.9
Release: 0%{?dist}
Summary: AWS S3 and Ceph command-line utility and benchmarking.

License: Apache2
Source0: s3lsio

%description
S3lsio is a command-line utility to access AWS S3 and Ceph. Also includes benchmarking.

%install
install -p -m 0755 %{SOURCE0} %{_bindir}

%files

%changelog
