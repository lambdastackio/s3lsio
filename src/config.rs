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

use url::Url;
use toml;

use lsio::error::{Error, Result};
use lsio::config::{ConfigFile, ParseInto};

/// Config by default is located at $HOME/.s3lsio/config for a given user. You can pass in an option
/// on the cli ```-c "<whatever path you want>"``` and it will override the default.
///
/// If for some reason there is no config file and nothing is passed in the all of the
/// fields will be None for Option values or whatever the defaults are for a given type.
///
///
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// endpoint is in the format <scheme>://<fqdn>:<port>
    pub endpoint: Option<Url>,
    /// proxy is in the format <scheme>://<fqdn>:<port>
    pub proxy: Option<Url>,
    /// signature is either V2 or V4
    pub signature: String,
}

impl ConfigFile for Config {
    type Error = Error;

    fn from_toml(toml: toml::Value) -> Result<Self> {
        let mut cfg = Config::default();

        try!(toml.parse_into("options.endpoint", &mut cfg.endpoint));
        try!(toml.parse_into("options.proxy", &mut cfg.proxy));
        try!(toml.parse_into("options.signature", &mut cfg.signature));

        Ok(cfg)
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            endpoint: None,
            proxy: None,
            signature: "V4".to_string(),
        }
    }
}

impl Config {
    pub fn set_endpoint(&mut self, value: Option<Url>) {
        self.endpoint = value;
    }

    pub fn set_proxy(&mut self, value: Option<Url>) {
        self.proxy = value;
    }

    pub fn set_signature(&mut self, value: String) {
        self.signature = value;
    }

    pub fn endpoint(&self) -> &Option<Url> {
        &self.endpoint
    }

    pub fn proxy(&self) -> &Option<Url> {
        &self.proxy
    }
}
