use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

macro_rules! valid_str {
    ($id:ident, $re:expr) => {
        #[derive(Debug, Serialize, Deserialize)]
        pub struct $id(String);

        impl $id {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Into<String> for $id {
            fn into(self) -> String {
                self.0
            }
        }

        /// `let name: Result<CLASS_NAME, ()> = "xxx".parse::<CLASS_NAME>();`
        impl FromStr for $id {
            type Err = ();
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                lazy_static! {
                    static ref RE: Regex = Regex::new($re).unwrap();
                }
                if RE.is_match(s) {
                    Ok(Self(s.to_string()))
                } else {
                    Err(())
                }
            }
        }
    };
}

valid_str!(ClassName, "^[0-9A-Za-z-]+$");
valid_str!(UserName, "^[0-9A-Za-z-]{5,}$");
valid_str!(UserPassword, "^[0-9A-Za-z-]{5,}$");
