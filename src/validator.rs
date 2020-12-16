use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Serialize, Deserialize)]
pub struct ClassName(String);

impl ClassName {
    pub fn from_string(s: String) -> Self {
        Self(s)
    }
    pub fn from_str(s: &str) -> Self {
        Self(s.to_owned())
    }
    pub fn get_name(&self) -> &str {
        &self.0
    }
}

impl FromStr for ClassName {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with('_') {
            Err(())
        } else {
            Ok(Self(s.to_string()))
        }
    }
}
