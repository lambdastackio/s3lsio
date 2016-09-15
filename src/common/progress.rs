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

use std::io::{self, Write};

use pbr;
use term;

pub enum Status {
    Applying,
    Cached,
    Creating,
    Downloading,
    Encrypting,
    Installed,
    Missing,
    Signing,
    Signed,
    Uploaded,
    Uploading,
    Using,
    Verified,
    Custom(char, String),
}

impl Status {
    pub fn parts(&self) -> (char, String, u16) {
        match *self {
            Status::Applying => ('↑', "Applying".into(), term::color::GREEN),
            Status::Cached => ('☑', "Cached".into(), term::color::GREEN),
            Status::Creating => ('Ω', "Creating".into(), term::color::GREEN),
            Status::Downloading => ('↓', "Downloading".into(), term::color::GREEN),
            Status::Encrypting => ('☛', "Encypting".into(), term::color::GREEN),
            Status::Installed => ('✓', "Installed".into(), term::color::GREEN),
            Status::Missing => ('∵', "Missing".into(), term::color::CYAN),
            Status::Signed => ('✓', "Signed".into(), term::color::CYAN),
            Status::Signing => ('☛', "Signing".into(), term::color::CYAN),
            Status::Uploaded => ('✓', "Uploaded".into(), term::color::GREEN),
            Status::Uploading => ('↑', "Uploading".into(), term::color::GREEN),
            Status::Using => ('→', "Using".into(), term::color::GREEN),
            Status::Verified => ('✓', "Verified".into(), term::color::GREEN),
            Status::Custom(c, ref s) => (c, s.to_string(), term::color::GREEN),
        }
    }
}

pub struct ProgressBar<T: Write> {
    bar: pbr::ProgressBar<T>,
    total: u64,
    current: u64,
}
