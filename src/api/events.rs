use juniper::FieldError;
use std::collections::HashMap;

use mongodb_base_service::{BaseService, ServiceError, ID};
use mongodb_cursor_pagination::FindResult;

use crate::db::{mongo::add_collection_by_name, Clients};
use crate::models::*;
use std::sync::RwLock;

lazy_static! {
    static ref CONFIGS: RwLock<HashMap<ID, Config>> = RwLock::new(HashMap::new());
}

/// Returns the string name of the collection
/// to use based on the application id and potentially a window of time.
fn get_collection_name(application_id: &ID, window: Option<&WindowType>) -> String {
    let app_id = format!("{}", application_id);

    match window {
        Some(window) => format!("{}_events_{}", app_id.to_ascii_lowercase(), window),
        None => format!("{}_all", application_id),
    }
}

fn is_valid_application(application_id: &ID) -> bool {
    let app_id = format!("{}", application_id);

    println!("app_id: {}", app_id.to_ascii_lowercase());
    println!("Configs: {:?}", CONFIGS.read().unwrap());
    match CONFIGS.read().unwrap().get(application_id) {
        Some(_config) => true,
        None => false,
    }
}

/// Sets the hashmap for the configurations
/// and also creates all of the needed database connections.
pub fn configure(clients: &mut Clients) -> Result<(), FieldError> {
    // start by reading all of the configs
    let config_service = &clients
        .mongo
        .get_mongo_service("configs")
        .expect("Unable to connect to database");
    let result: FindResult<Config> = config_service.find(None, None, None, None, None, None)?;

    let mut configs = CONFIGS.write().unwrap();
    result.items.iter().for_each(|config| {
        // store these for quick access
        configs.insert(config.application_id.clone(), config.clone());
        // add a database configuration for all variations
        // appid_events_all
        // appid_events_hour, appid_events_day, etc...
        let name = get_collection_name(&config.application_id, None);
        add_collection_by_name(&mut clients.mongo, &name);

        // add database configurations for the windows
        config.windows.iter().for_each(|window| {
            let name = get_collection_name(&config.application_id, Some(window));
            add_collection_by_name(&mut clients.mongo, &name);
        });
    });

    Ok(())
}

pub fn all_events(
    ctx: &Clients,
    application_id: &ID,
    limit: Option<i32>,
    after: Option<String>,
    before: Option<String>,
    skip: Option<i32>,
) -> Result<EventConnection, FieldError> {
    if !is_valid_application(application_id) {
        return Err("Invalid application ID".into());
    }

    let collection_name = get_collection_name(application_id, None);
    let service = &ctx.mongo.get_mongo_service(&collection_name).unwrap();
    let result: Result<FindResult<Event>, ServiceError> =
        service.find(None, None, limit, after, before, skip);
    match result {
        Ok(all_items) => {
            let connection: EventConnection = all_items.into();
            Ok(connection)
        }
        Err(e) => Err(FieldError::from(e)),
    }
}

/// This just returns the event back, the storing of the event happens separately
///
/// However, it will check to see if the application_id is valid or not and return an error
pub fn log_event(
    ctx: &Clients,
    application_id: &ID,
    new_event: NewEvent,
    created_by_id: Option<ID>,
) -> Result<Event, FieldError> {
    if !is_valid_application(application_id) {
        return Err("Invalid application ID".into());
    }

    let collection_name = get_collection_name(application_id, None);
    let service = &ctx.mongo.get_mongo_service(&collection_name).unwrap();

    let inserted_id: ID = service.insert_one(new_event, created_by_id)?;
    let maybe_item = service.find_one_by_id(inserted_id)?;
    match maybe_item {
        Some(item) => Ok(item),
        None => Err("Unable to retrieve object after insert".into()),
    }
}
