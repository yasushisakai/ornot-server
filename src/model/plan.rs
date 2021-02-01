use crate::model::Settable;
use bs58::encode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Debug;

// a plan is one option within a topic that could be voted to.
// We are avoiding the word 'option', not to coflict with Rust's built in Option enum

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Plan {
    #[serde(rename = "simple")]
    Simple(String),
    #[serde(rename = "long")]
    Long(String, String),
    #[serde(rename = "url")]
    Url(String, String), // title, url
    #[serde(rename = "image")]
    Image(String), // url
    #[serde(rename = "latlng")]
    LatLng(Option<String>, Location),
    #[serde(rename = "circle")]
    Circle(Option<String>, CircularArea),
    #[serde(rename = "path")]
    Path(Option<String>, Vec<(f64, f64)>),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Location {
    lat: f64,
    lng: f64,
}

impl std::fmt::Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}, {}", self.lat, self.lng)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CircularArea {
    lat: f64,
    lng: f64,
    radius: f64,
}

impl std::fmt::Display for CircularArea {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}, {}, {}", self.lat, self.lng, self.radius)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RawPlan {
    #[serde(rename = "type")]
    kind: String,
    data: serde_json::Value,
}

impl From<RawPlan> for Plan {
    fn from(raw: RawPlan) -> Self {
        match raw.kind.to_lowercase().as_str() {
            "simple" => {
                let title: String =
                    serde_json::from_str(&raw.data.to_string()).expect("should be just a string");
                Plan::Simple(title)
            }
            "long" => {
                let (title, description): (String, String) =
                    serde_json::from_str(&raw.data.to_string())
                        .expect("this should be deserializable");
                Plan::Long(title, description)
            }
            "url" => {
                let (title, url): (String, String) = serde_json::from_str(&raw.data.to_string())
                    .expect("this should be deserializable");
                Plan::Url(title, url)
            }
            "image" => {
                let url: String =
                    serde_json::from_str(&raw.data.to_string()).expect("should be just a string");
                Plan::Image(url)
            }
            "latlng" => {
                let (name, location): (Option<String>, Location) =
                    serde_json::from_str(&raw.data.to_string())
                        .expect("this should be Deserialize");
                Plan::LatLng(name, location)
            }
            "circle" => {
                let (name, area): (Option<String>, CircularArea) =
                    serde_json::from_str(&raw.data.to_string())
                        .expect("this should be Deserialize");
                Plan::Circle(name, area)
            }
            "path" => {
                let (name, indices): (Option<String>, Vec<(f64, f64)>) =
                    serde_json::from_str(&raw.data.to_string())
                        .expect("this should be Deserialize");
                Plan::Path(name, indices)
            }
            _ => Plan::Simple("unknown type".into()),
        }
    }
}

impl Settable for Plan {
    fn domain_prefix() -> String {
        "plan".to_string()
    }

    fn id(&self) -> String {
        match &self {
            Plan::Simple(x) | Plan::Long(x, _) | Plan::Image(x) => {
                encode(Sha256::digest(x.as_bytes())).into_string()
            }
            Plan::Url(x, y) => {
                let cat = format!("{}{}", x, y);
                encode(Sha256::digest(cat.as_bytes())).into_string()
            }
            Plan::LatLng(_name, location) => {
                let cat = location.to_string();
                encode(Sha256::digest(cat.as_bytes())).into_string()
            }
            Plan::Circle(_name, area) => {
                let cat = area.to_string();
                encode(Sha256::digest(cat.as_bytes())).into_string()
            }
            Plan::Path(_name, indices) => {
                let cat = indices.iter().fold(String::new(), |acc, v| format!("{:?}{:?}", acc, v) );
                encode(Sha256::digest(cat.as_bytes())).into_string()
            }
        }
    }

    fn list_item(&self) -> String {
        let entry = match &self {
            Plan::Simple(x) | Plan::Image(x) => {
                let x = x.to_string();
                vec![self.id(), x]
            }
            Plan::Long(x, _y) | Plan::Url(x, _y) => {
                let id = self.id();
                vec![id, x.to_string()]
            }
            Plan::LatLng(_name, location) => {
                let id = self.id();
                vec![id, location.to_string()]
            }
            Plan::Circle(_name, area) => {
                let id = self.id();
                vec![id, area.to_string()]
            }
            Plan::Path(name, _indices) => {
                let id = self.id();
                let info = match name {
                    Some(v) => v,
                    None => ""
                };
                vec![id, info.to_string()]
            }
        };

        serde_json::to_string(&entry).expect("should be Serializable")
    }

    fn prefix(&self) -> String {
        Self::domain_prefix()
    }

    fn domain(&self) -> String {
        return format!("{}:{}", Self::domain_prefix(), &self.id());
    }

    fn json(&self) -> String {
        serde_json::to_string(&self).expect("I should be Serialize-able")
    }
}
