// use bs58::encode;
use serde::{Deserialize, Serialize};
// use sha2::{Digest, Sha256};
use std::fmt::Debug;
use crate::model::Settable;

// a plan is one option within a topic that could be voted to.
// We are avoiding the word 'option', not to coflict with Rust's built in Option enum

#[derive(Debug, Serialize, Deserialize)]
pub struct SimplePlan(pub String);

impl Settable for SimplePlan {
    fn domain_prefix() -> String {
        String::from("plan")
    }

    fn id(&self) -> String {
        self.0.to_string()
    }
    
    fn list_item(&self) -> String {
        self.id()
    }
}



