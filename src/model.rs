use bs58::encode;
use liq::{Plan, PollResult, Setting};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fmt::Debug;

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
    pub fn add_plan(&mut self, text: String) {
        self.setting.add_plan(Plan::new(text));
    }

    pub fn remove_plan(&mut self, text: String) {
        self.setting.delete_plan(&text);
    }

    pub fn get_users(&self) -> HashSet<String> {
        self.setting.get_voters()
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

#[derive(Debug, Deserialize, Serialize)]
pub struct PartialUser {
    pub nickname: String,
    pub email: String,
}

impl From<PartialUser> for User {
    fn from(p_user: PartialUser) -> User {
        User::new(p_user.nickname, p_user.email)
    }
}

impl Settable for User {
    fn domain_prefix() -> String {
        String::from("user")
    }

    fn id(&self) -> String {
        self.id.to_string()
    }

    fn list_item(&self) -> String {
        serde_json::to_string(&vec![&self.id, &self.nickname])
            .expect("User-Settable should be Serializable")
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct User {
    id: String,
    pub nickname: String,
    pub is_verified: bool,
}

impl User {
    pub fn new(nickname: String, email: String) -> Self {
        let cat = format!("email:{}", &email);
        let id = encode(Sha256::digest(&cat.as_bytes())).into_string();

        Self {
            id,
            nickname,
            is_verified: false,
        }
    }

    pub fn json(&self) -> String {
        serde_json::to_string(self).expect("user should be able to serialize")
    }
}
