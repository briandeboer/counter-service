use mongodb_base_service::ID;
use mongodb_cursor_pagination::{Edge, FindResult, PageInfo};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

use crate::schema::Context;

#[derive(juniper::GraphQLEnum, Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum WindowType {
    Hour,
    Day,
    Week,
    Month,
    AllTime,
}

impl Display for WindowType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        match *self {
            WindowType::Hour => f.write_str("hour"),
            WindowType::Day => f.write_str("day"),
            WindowType::Week => f.write_str("week"),
            WindowType::Month => f.write_str("month"),
            WindowType::AllTime => f.write_str("alltime"),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Bucket {
    #[serde(rename = "_id")] // Use MongoDB's special primary key field name when serializing
    pub hash: ID,
    pub application_id: ID,
    pub grouping: String,
    pub nested_groupings: Vec<String>,
    pub window: WindowType,
    pub timestamp: i32,
    pub events: Option<Vec<EmbeddedEvent>>,
    pub event_ids: Option<Vec<ID>>,
    pub count: i32,
}

#[juniper::object(context = Context, description = "All the events grouped")]
impl Bucket {
    pub fn hash(&self) -> &ID {
        &self.hash
    }

    fn application_id(&self) -> &ID {
        &self.application_id
    }

    fn grouping(&self) -> &str {
        &self.grouping
    }

    fn nested_groupings(&self) -> &Vec<String> {
        &self.nested_groupings
    }

    fn window(&self) -> &WindowType {
        &self.window
    }

    fn timestamp(&self) -> i32 {
        self.timestamp
    }

    fn count(&self) -> i32 {
        self.count
    }

    fn events(&self, limit: Option<i32>, skip: Option<i32>) -> Vec<EmbeddedEvent> {
        match &self.events {
            Some(events) => {
                let len = events.len();
                let skip = skip.unwrap_or(0) as usize;
                if skip > len {
                    return vec![];
                }
                let mut limit = limit.unwrap_or(10) as usize;
                if skip + limit > len {
                    limit = len - skip;
                }
                let truncated: Vec<EmbeddedEvent> = events[skip..(skip + limit)].into();
                truncated
            }
            None => vec![],
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BucketConnection {
    pub page_info: PageInfo,
    pub edges: Vec<Edge>,
    pub items: Vec<Bucket>,
    pub total_count: i64,
}

#[juniper::object(Context = Context)]
impl BucketConnection {
    fn page_info(&self) -> &PageInfo {
        &self.page_info
    }

    fn edges(&self) -> &Vec<Edge> {
        &self.edges
    }

    fn items(&self) -> &Vec<Bucket> {
        &self.items
    }

    fn total_count(&self) -> i32 {
        self.total_count as i32
    }
}

impl From<FindResult<Bucket>> for BucketConnection {
    fn from(fr: FindResult<Bucket>) -> BucketConnection {
        BucketConnection {
            page_info: fr.page_info,
            edges: fr.edges,
            items: fr.items,
            total_count: fr.total_count,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, juniper::GraphQLObject)]
pub struct EmbeddedEvent {
    pub timestamp: i32,
    pub raw_timestamp: i32,
    #[serde(alias = "ipaddress")]
    pub ip_address: Option<String>,
    #[serde(alias = "eventtype")]
    pub event_type: Option<String>,
}
