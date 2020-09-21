use actix_web::web::Data;
use bson::doc;
use cached::TimedCache;
use juniper::{FieldError, RootNode};
use jwt_validator::{Claims, TestClaims};
use log::debug;
use mongodb_base_service::{BaseService, ServiceError, ID};
use mongodb_cursor_pagination::FindResult;
use std::env;
use std::sync::Arc;
use std::time::SystemTime;

use crate::api;
use crate::db::Clients;
use crate::models::*;

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

lazy_static! {
    static ref CACHE_CAPACITY: usize = env::var("CACHE_CAPACITY")
        .unwrap_or("".to_string())
        .parse()
        .unwrap_or(10000);
    static ref CACHE_TTL: u64 = env::var("CACHE_TTL")
        .unwrap_or("".to_string())
        .parse()
        .unwrap_or(60);
    static ref DISABLE_AUTH: u8 = env::var("DISABLE_AUTH")
        .unwrap_or("".to_string())
        .parse()
        .unwrap_or(0);
    static ref REQUIRED_EMAIL_DOMAIN: String =
        env::var("REQUIRED_EMAIL_DOMAIN").unwrap_or("gmail.com".to_string());
}

pub struct Context {
    pub clients: Data<Arc<Clients>>,
    pub claims: Option<Claims>,
}

impl juniper::Context for Context {}

pub struct Query;

#[juniper::object(Context = Context)]
impl Query {
    fn all_configs(
        ctx: &Context,
        limit: Option<i32>,
        after: Option<String>,
        before: Option<String>,
        skip: Option<i32>,
    ) -> Result<ConfigConnection, FieldError> {
        debug!("Building all configs");
        let service = ctx
            .clients
            .get_ref()
            .mongo
            .get_mongo_service("configs")
            .unwrap();
        let result: Result<FindResult<Config>, ServiceError> =
            service.find(None, None, limit, after, before, skip);
        match result {
            Ok(all_items) => {
                let connection: ConfigConnection = all_items.into();
                Ok(connection)
            }
            Err(e) => Err(FieldError::from(e)),
        }
    }

    fn all_events(
        ctx: &Context,
        application_id: ID,
        limit: Option<i32>,
        after: Option<String>,
        before: Option<String>,
        skip: Option<i32>,
    ) -> Result<EventConnection, FieldError> {
        api::events::all_events(
            ctx.clients.get_ref(),
            &application_id,
            limit,
            after,
            before,
            skip,
        )
    }
}

pub struct Mutation;

fn has_auth(ctx: &Context) -> bool {
    if ctx.claims.is_none()
        || !ctx.claims.clone().unwrap().validate(TestClaims {
            hd: Some(REQUIRED_EMAIL_DOMAIN.to_string()),
            ..TestClaims::default()
        })
    {
        return false;
    }
    return true;
}

#[juniper::object(Context = Context)]
impl Mutation {
    // samples
    fn log_event(
        ctx: &Context,
        application_id: ID,
        new_event: NewEvent,
        created_by_id: Option<ID>,
    ) -> Result<Event, FieldError> {
        api::events::log_event(
            ctx.clients.get_ref(),
            &application_id,
            new_event,
            created_by_id,
        )
    }
}

pub type Schema = RootNode<'static, Query, Mutation>;

#[allow(dead_code)]
pub fn create_schema() -> Schema {
    Schema::new(Query {}, Mutation {})
}
