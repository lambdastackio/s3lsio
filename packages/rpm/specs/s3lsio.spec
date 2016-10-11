Name:    s3lsio
Version: 0.1.7
Release: 7%{?dist}
Summary: AWS S3 and Ceph command-line utility

License: Apache2
Source0: s3lsio

%description
S3lsio is a command-line utility to access AWS S3 and Ceph.

%install
install -p -m 0755 %{SOURCE0} %{_bindir}

%files

%changelog
