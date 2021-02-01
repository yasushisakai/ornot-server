use bs58::encode;
use liq::{PollResult, Setting};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::Debug;
use crate::model::Settable;

#[derive(Debug, Serialize, Deserialize)]
pub struct Topic {
    id: String,
    title: String,
    description: String,
    pub setting_hash: String,
    setting_prev_hash: String,
    setting: Setting,
    result: Option<PollResult>,
}

impl Settable for Topic {
    fn domain_prefix() -> String {
        String::from("topic")
    }

    fn id(&self) -> String {
        self.id.to_string()
    }

    fn list_item(&self) -> String {
        serde_json::to_string(&vec![&self.id, &self.title]).expect("should be Serializable")
    }
}

impl Topic {
    pub fn add_plan_id(&mut self, plan_id: &str) {
        self.setting.add_plan(plan_id);
    }

    pub fn remove_plan_id(&mut self, plan_id: &str) {
        self.setting.delete_plan(plan_id);
    }

    pub fn add_user(&mut self, user_id: String) {
        self.setting.add_voter(&user_id);
    }

    pub fn remove_user(&mut self, user_id: String) {
        self.setting.delete_voter(&user_id);
    }

    pub fn setting_json(&self) -> Vec<u8> {
        serde_json::to_vec(&self.setting)
            .expect("Topic's Setting should be able to be Serialized")
    }

    pub fn insert_vote(&mut self, user_id: &str, vote: BTreeMap<String, f64>) -> String {
        // more like swapping the HashMap
        self.setting.overwrite_vote(user_id, vote);
        self.setting.based_hash()
    }

    pub fn update_setting_hash(&mut self, new_hash: &str) {
        self.setting_prev_hash = self.setting_hash.to_owned();
        self.setting_hash = new_hash.to_string();
    }

    pub fn calculate(&mut self) {
        let result = self.setting.calculate();
        self.result = Some(result);
    }
}

#[derive(Deserialize, Debug)]
pub struct PartialTopic {
    title: String,
    description: String,
}

impl From<PartialTopic> for Topic {
    fn from(p_topic: PartialTopic) -> Self {
        let cat = format!("{}{}", p_topic.title, p_topic.description);
        let id = encode(Sha256::digest(&cat.as_bytes())).into_string();

        Self {
            id,
            title: p_topic.title,
            description: p_topic.description,
            setting_hash: "0".to_string(),
            setting: Setting::new(),
            setting_prev_hash: "0".to_string(),
            result: None,
        }
    }
}


