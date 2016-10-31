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

// Will remove later
#![allow(dead_code)]

use std::io::{self, Write};

use pbr;
use term;

pub enum Status {
    Getting,
    GettingRange,
    Putting,
    Custom(char, String),
}

impl Status {
    pub fn parts(&self) -> (char, String, u16) {
        match *self {
            Status::Getting => ('↓', "Getting".into(), term::color::GREEN),
            Status::GettingRange => ('→', "GettingRange".into(), term::color::GREEN),
            Status::Putting => ('↑', "Putting".into(), term::color::GREEN),
            Status::Custom(c, ref s) => (c, s.to_string(), term::color::GREEN),
        }
    }
}

pub struct ProgressBar<T: Write> {
    bar: pbr::ProgressBar<T>,
    total: u64,
    current: u64,
}
