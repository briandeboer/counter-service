use bson::doc;
use chrono::{DateTime, Utc};
use mongodb_base_service::{Node, NodeDetails, ID};
use mongodb_cursor_pagination::{Edge, FindResult, PageInfo};
use serde::{Deserialize, Serialize};

use crate::models::WindowType;
use crate::schema::Context;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(rename = "_id")] // Use MongoDB's special primary key field name when serializing
    pub application_id: ID,
    pub node: NodeDetails,
    pub windows: Vec<WindowType>,
    pub groups: Vec<String>,
}

impl Node for Config {
    fn node(&self) -> &NodeDetails {
        &self.node
    }
}

#[juniper::object(Context = Context, description = "Config model")]
impl Config {
    fn application_id(&self) -> &ID {
        &self.application_id
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

    fn windows(&self) -> &Vec<WindowType> {
        &self.windows
    }

    fn groups(&self) -> &Vec<String> {
        &self.groups
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ConfigConnection {
    pub page_info: PageInfo,
    pub edges: Vec<Edge>,
    pub items: Vec<Config>,
    pub total_count: i64,
}

#[juniper::object(Context = Context)]
impl ConfigConnection {
    fn page_info(&self) -> &PageInfo {
        &self.page_info
    }

    fn edges(&self) -> &Vec<Edge> {
        &self.edges
    }

    fn items(&self) -> &Vec<Config> {
        &self.items
    }

    fn total_count(&self) -> i32 {
        self.total_count as i32
    }
}

impl From<FindResult<Config>> for ConfigConnection {
    fn from(fr: FindResult<Config>) -> ConfigConnection {
        ConfigConnection {
            page_info: fr.page_info,
            edges: fr.edges,
            items: fr.items,
            total_count: fr.total_count,
        }
    }
}

#[derive(Serialize, Deserialize, juniper::GraphQLInputObject)]
pub struct NewConfig {
    #[serde(rename = "_id")]
    pub application_id: ID,
    pub windows: Vec<WindowType>,
    pub groups: Vec<String>,
}
