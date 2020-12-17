use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Serialize, Deserialize)]
pub struct ClassName(String);

impl ClassName {
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

#[derive(Debug, Serialize, Deserialize)]
pub struct UserName(String);

impl UserName {
    pub fn get_name(&self) -> &str {
        &self.0
    }
}

impl FromStr for UserName {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with('_') {
            Err(())
        } else {
            Ok(Self(s.to_string()))
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserPassword(String);

impl UserPassword {
    pub fn get_name(&self) -> &str {
        &self.0
    }
}

impl FromStr for UserPassword {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with('_') {
            Err(())
        } else {
            Ok(Self(s.to_string()))
        }
    }
}
