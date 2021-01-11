use sha2::{Sha256, Digest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use liq::{Setting, Policy, PollResult, create_matrix, calculate, poll_result};

impl Settable for Setting {
    fn domain(&self) -> String {
       let votes = serde_json::to_vec(&self.votes).unwrap(); 
       format!("votes:{:x}", Sha256::digest(&votes))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Topic {
    pub id: String,
    title: String,
    description: String,
    setting_id: Option<String>,
    setting: Setting,
    pub result: Option<PollResult>
}

impl Settable for Topic {
    fn domain(&self) -> String {
        format!("topic:{}",self.id)
    }
}

type Vote = HashMap<String, f64>;

impl Topic {
    pub fn add_plan(&mut self, text: String) {
        let policy = Policy::Short(text);
        self.setting.add_policy(policy);
    }

    pub fn delete_plan(&mut self, text: String) {
        self.setting.delete_policy(&text);
    }

    pub fn add_user(&mut self, user_id: String) {
        self.setting.add_voter(&user_id);
    }

    pub fn delete_user(&mut self, user_id: String) {
        self.setting.delete_voter(&user_id);
    }

    pub fn insert_vote(&mut self, user_id: String, vote: Vote) {
        // if I cast a vote, should I be in voters?
        // for now I should be registered as voters first...
        
        if self.setting.voters.iter().any(|voter| voter==&user_id) {
            self.setting.votes.insert(user_id, vote);
        }

        // otherwise do nothing

    }

    pub fn calculate_result(&mut self) {
        let domain = self.setting.domain();
        if self.setting_id == None || self.setting_id != Some(domain) {
            let m = create_matrix(&self.setting);
            let r = calculate(m, self.setting.voters.len());
            let result = poll_result(&self.setting.voters, &self.setting.policies, r);
            self.result = Some(result);
            self.setting_id = Some(self.setting.domain());
        }
    }
}

#[derive(Deserialize)]
pub struct PartialTopic {
    title: String,
    description: String,
}

impl From<PartialTopic> for Topic {
    fn from (p_topic: PartialTopic) -> Self {
        let cat = format!("{}{}", p_topic.title, p_topic.description);
        let h = Sha256::digest(&cat.as_bytes());

        Self{
            id: format!("{:x}", h),
            title: p_topic.title,
            description: p_topic.description,
            setting_id: None,
            setting: Setting::new(),
            result: None,
        } 
    }
}

pub trait Settable {
    fn domain(&self) -> String;
}

#[derive(Deserialize, Serialize)]
pub struct User {
    id: String,
    nickname: String,
    email: String
}

#[derive(Deserialize, Serialize)]
pub struct PartialUser {
    nickname: String,
    email: String,
}

impl From<PartialUser> for User {
    fn from(p_user:PartialUser) -> User {
        User::new(p_user.nickname, p_user.email)
    }
}

impl User {
    fn new(nickname: String, email: String) -> Self {
        let cat = format!("{}{}", &nickname, &email);
        let h = Sha256::digest(&cat.as_bytes());
        let id = format!("{:x}", h);

        Self {
            id, nickname, email
        }
    }

    // for debugging reasons, this function should go away once everything is working
    fn new_with_id(nickname: String, email: String, id: String) -> Self {
        Self{
            id,
            nickname,
            email
        }
    }
}

impl Settable for User {
    fn domain(&self) -> String {
        format!("user:{}", self.id)
    }
}

