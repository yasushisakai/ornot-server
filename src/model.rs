use bs58::encode;
use liq::{Plan, PollResult, Setting};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
// use actix_web::{web, Error as AWError};
// use actix::Addr;
// use actix_redis::{Command, RedisActor};
// use redis_async::{resp_array, resp::RespValue};

impl Settable for Setting {
    fn domain(&self) -> String {
        let based_hash = self.based_hash();
        format!("setting:{}", based_hash)
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
    fn domain(&self) -> String {
        format!("topic:{}", self.id)
    }
}

impl Topic {
    pub fn list_item(&self) -> (String, String) {
        (self.id.to_owned(), self.title.to_owned())
    }

    pub fn json(&self) -> String {
        serde_json::to_string(self).expect("should be able to Serialize")
    }

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
        serde_json::to_vec(&self.setting).expect("Topic's Setting should be able to be Serialized")
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

    pub fn result_domain(&self) -> String {
        let result: &PollResult = self
            .result
            .as_ref()
            .expect("we should have a valid result id");

        format!("result:{}", result.based_hash())
    }
}

#[derive(Deserialize)]
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

pub trait Settable {
    fn domain(&self) -> String;
}

#[derive(Deserialize, Serialize)]
pub struct PartialUser {
    pub nickname: String,
    pub email: String,
}

impl From<PartialUser> for User {
    fn from(p_user: PartialUser) -> User {
        User::new(p_user.nickname, p_user.email)
    }
}

#[derive(Deserialize, Serialize)]
pub struct User {
    pub id: String,
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
            // email,
            // temp_code: None,
            // access_token: None,
            is_verified: false,
        }
    }

    pub fn json(&self) -> String {
        serde_json::to_string(self).expect("user should be able to serialize")
    }
}

impl Settable for User {
    fn domain(&self) -> String {
        format!("user:{}", self.id)
    }
}
