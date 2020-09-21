use bson::doc;
use chrono::{DateTime, Utc};
use juniper::{GraphQLInputObject, GraphQLObject};
use mongodb_base_service::{Node, NodeDetails, ID};
use mongodb_cursor_pagination::{Edge, FindResult, PageInfo};
use serde::{Deserialize, Serialize};

use crate::schema::Context;

#[derive(Clone, Serialize, Deserialize)]
pub struct Event {
    #[serde(rename = "_id")] // Use MongoDB's special primary key field name when serializing
    pub id: ID,
    pub keys: Vec<KeyPair>,
    pub node: NodeDetails,
    pub timestamp: i32,
}

impl Node for Event {
    fn node(&self) -> &NodeDetails {
        &self.node
    }
}

#[juniper::object(Context = Context, description = "Event model")]
impl Event {
    fn id(&self) -> &ID {
        &self.id
    }

    fn date_created(&self) -> Option<DateTime<Utc>> {
        self.node.date_created()
    }

    fn date_modified(&self) -> Option<DateTime<Utc>> {
        self.node.date_modified()
    }

    fn created_by(&self) -> Option<&ID> {
        match self.node.created_by_id() {
            Some(id) => Some(id),
            None => None,
        }
    }

    fn updated_by(&self) -> Option<&ID> {
        match self.node.updated_by_id() {
            Some(id) => Some(id),
            None => None,
        }
    }

    fn keys(&self) -> &Vec<KeyPair> {
        &self.keys
    }

    fn timestamp(&self) -> &i32 {
        &self.timestamp
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EventConnection {
    pub page_info: PageInfo,
    pub edges: Vec<Edge>,
    pub items: Vec<Event>,
    pub total_count: i64,
}

#[juniper::object(Context = Context)]
impl EventConnection {
    fn page_info(&self) -> &PageInfo {
        &self.page_info
    }

    fn edges(&self) -> &Vec<Edge> {
        &self.edges
    }

    fn items(&self) -> &Vec<Event> {
        &self.items
    }

    fn total_count(&self) -> i32 {
        self.total_count as i32
    }
}

impl From<FindResult<Event>> for EventConnection {
    fn from(fr: FindResult<Event>) -> EventConnection {
        EventConnection {
            page_info: fr.page_info,
            edges: fr.edges,
            items: fr.items,
            total_count: fr.total_count,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, juniper::GraphQLInputObject)]
pub struct NewEvent {
    #[serde(rename = "_id")]
    id: Option<ID>,
    pub keys: Vec<NewKeyPair>,
    pub timestamp: i32,
}

pub trait KeyPairing {
    fn key(&self) -> String;
    fn value(&self) -> String;
    fn lowercase(&self) -> Self;
}

#[derive(Clone, Serialize, Deserialize, GraphQLObject)]
pub struct KeyPair {
    pub key: String,
    pub value: String,
}

impl KeyPairing for KeyPair {
    fn key(&self) -> String {
        self.key.to_ascii_lowercase()
    }

    fn value(&self) -> String {
        self.value.to_ascii_lowercase()
    }

    fn lowercase(&self) -> Self {
        Self {
            key: self.key(),
            value: self.value(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, GraphQLInputObject)]
pub struct NewKeyPair {
    pub key: String,
    pub value: String,
}

impl KeyPairing for NewKeyPair {
    fn key(&self) -> String {
        self.key.to_ascii_lowercase()
    }

    fn value(&self) -> String {
        self.value.to_ascii_lowercase()
    }

    fn lowercase(&self) -> Self {
        Self {
            key: self.key(),
            value: self.value(),
        }
    }
}
