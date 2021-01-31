mod user;
mod topic;
mod plan;

use liq::Setting;
use serde::Serialize;
use std::fmt::Debug;

pub use user::{User, PartialUser};
pub use topic::{Topic, PartialTopic};
pub use plan::SimplePlan;

pub trait Settable: Serialize + Debug {
    fn domain_prefix() -> String;

    fn prefix(&self) -> String {
        Self::domain_prefix()
    } //FIXME: theres got be a smarter way...

    fn id(&self) -> String;
    fn list_item(&self) -> String;

    fn domain(&self) -> String {
        return format!("{}:{}", Self::domain_prefix(), &self.id());
    }

    fn json(&self) -> String {
        serde_json::to_string(&self).expect("I should be Serialize-able")
    }
}

impl Settable for Setting {
    fn domain_prefix() -> String {
        String::from("setting")
    }
    fn id(&self) -> String {
        self.based_hash()
    }

    fn list_item(&self) -> String {
        self.id()
    }
}


