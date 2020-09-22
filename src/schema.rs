use actix_web::web::Data;
use bson::doc;
use juniper::{FieldError, RootNode};
use jwt_validator::{Claims, TestClaims};
use log::debug;
use mongodb_base_service::{BaseService, DeleteResponseGQL, ServiceError, ID};
use mongodb_cursor_pagination::FindResult;
use std::env;
use std::sync::Arc;
use std::time::SystemTime;

use crate::api;
use crate::api::lowercase_id;
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

    fn config_by_application_id(ctx: &Context, application_id: ID) -> Result<Config, FieldError> {
        let service = ctx
            .clients
            .get_ref()
            .mongo
            .get_mongo_service("configs")
            .unwrap();
        let result: Result<Option<Config>, ServiceError> = service.find_one_by_id(application_id);
        match result {
            Ok(item) => match item {
                Some(item) => Ok(item),
                None => Err("Unable to find item".into()),
            },
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

    fn event_group_by_keys(
        ctx: &Context,
        application_id: ID,
        window: WindowType,
        timestamp: i32,
        group: String,
        keys: Vec<NewKeyPair>,
    ) -> Result<Bucket, FieldError> {
        api::events::bucket_by_keys(
            ctx.clients.get_ref(),
            &application_id,
            &window,
            timestamp,
            &group,
            &keys,
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
    // configs
    fn create_config(
        ctx: &Context,
        mut new_config: NewConfig,
        created_by_id: Option<ID>,
    ) -> Result<Config, FieldError> {
        if !has_auth(ctx) && *DISABLE_AUTH != 1 {
            return Err("Unauthorized".into());
        }
        let service = ctx.clients.mongo.get_mongo_service("configs").unwrap();
        new_config.application_id = lowercase_id(&new_config.application_id);
        new_config.groups = new_config
            .groups
            .iter()
            .map(|g| g.to_ascii_lowercase())
            .collect();
        let inserted_id: ID = service.insert_one(new_config, created_by_id)?;
        let maybe_item = service.find_one_by_id(inserted_id)?;
        match maybe_item {
            Some(item) => Ok(item),
            None => Err("Unable to retrieve object after insert".into()),
        }
    }

    fn update_config(
        ctx: &Context,
        application_id: ID,
        mut update_config: UpdateConfig,
        updated_by_id: Option<ID>,
    ) -> Result<Config, FieldError> {
        if !has_auth(ctx) && *DISABLE_AUTH != 1 {
            return Err("Unauthorized".into());
        }
        // check authorization first
        let service = ctx.clients.mongo.get_mongo_service("configs").unwrap();
        // lowercase all the groups
        if let Some(groups) = update_config.groups {
            update_config.groups = Some(groups.iter().map(|g| g.to_ascii_lowercase()).collect());
        }
        service
            .update_one(lowercase_id(&application_id), update_config, updated_by_id)
            .map_err(|e| e.into())
    }

    fn delete_config(ctx: &Context, application_id: ID) -> Result<DeleteResponseGQL, FieldError> {
        if !has_auth(ctx) && *DISABLE_AUTH != 1 {
            return Err("Unauthorized".into());
        }
        let service = ctx.clients.mongo.get_mongo_service("configs").unwrap();
        match service.delete_one_by_id(lowercase_id(&application_id)) {
            Ok(result) => Ok(result.into()),
            Err(e) => Err(e.into()),
        }
    }

    // events
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
