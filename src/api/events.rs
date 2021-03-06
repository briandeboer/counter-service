use bson::doc;
use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike, Weekday};
use juniper::FieldError;
use mongodb::options::UpdateOptions;
use mongodb_base_service::{BaseService, ServiceError, ID};
use mongodb_cursor_pagination::FindResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

use crate::api::lowercase_id;
use crate::db::{mongo::add_collection_by_name, Clients};
use crate::models::*;
use crate::schema::now;

lazy_static! {
    static ref CONFIGS: RwLock<HashMap<ID, Config>> = RwLock::new(HashMap::new());
}

/// Returns the string name of the collection
/// to use based on the application id and potentially a window of time.
fn get_collection_name(application_id: &ID, window: Option<&WindowType>) -> String {
    let application_id = lowercase_id(application_id);
    match window {
        Some(window) => format!("{}_events_{}", application_id, window),
        None => format!("{}_all", application_id),
    }
}

/// Returns the start timestamp based on the window
fn get_timestamp_start(window: &WindowType, timestamp: i32) -> i32 {
    let dt = NaiveDateTime::from_timestamp(timestamp as i64, 0);
    match window {
        WindowType::Hour => dt.date().and_hms(dt.hour(), 0, 0).timestamp() as i32,
        WindowType::Day => dt.date().and_hms(0, 0, 0).timestamp() as i32,
        WindowType::Week => {
            let start_of_week = dt.date().iso_week();
            NaiveDate::from_isoywd(start_of_week.year(), start_of_week.week(), Weekday::Mon)
                .and_hms(0, 0, 0)
                .timestamp() as i32
        }
        WindowType::Month => NaiveDate::from_ymd(dt.year(), dt.month(), 0)
            .and_hms(0, 0, 0)
            .timestamp() as i32,
        WindowType::AllTime => -1,
    }
}

fn get_hash_id(
    window: &WindowType,
    group_def: &str,
    keypairs: &Vec<impl KeyPairing>,
    timestamp: i32,
) -> String {
    format!(
        "{}|{}|{}",
        window,
        timestamp,
        get_group_id(group_def, keypairs)
    )
}

fn get_group_id(group_def: &str, keypairs: &Vec<impl KeyPairing>) -> String {
    group_def.split('|').fold("".to_string(), |mut acc, key| {
        let maybe_keypair = keypairs
            .iter()
            .find(|k| k.key() == key.to_ascii_lowercase());
        let value = match maybe_keypair {
            Some(kp) => kp.value(),
            None => "null".to_string(),
        };
        if acc == "" {
            acc = value.clone();
        } else {
            acc = format!("{}|{}", acc, value);
        }
        acc
    })
}

pub fn is_valid_application(application_id: &ID) -> bool {
    match CONFIGS.read().unwrap().get(&lowercase_id(application_id)) {
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

pub fn bucket_by_keys(
    ctx: &Clients,
    application_id: &ID,
    window: &WindowType,
    timestamp: i32,
    group_def: &str,
    keypairs: &Vec<NewKeyPair>,
) -> Result<Bucket, FieldError> {
    if !is_valid_application(application_id) {
        return Err("Invalid application ID".into());
    }

    let collection_name = get_collection_name(application_id, Some(window));
    let service = &ctx.mongo.get_mongo_service(&collection_name).unwrap();
    let start_timestamp = get_timestamp_start(window, timestamp);
    let hash = get_hash_id(window, group_def, keypairs, start_timestamp);

    println!("hash: {:?}", ID::from(hash.clone()));
    let result: Option<Bucket> = service.find_one_by_id(ID::from(hash))?;
    match result {
        Some(bucket) => Ok(bucket),
        None => Err("Unable to find event group".into()),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, juniper::GraphQLObject)]
pub struct CountResponse {
    total_record_count: i32,
    total_aggregate_count: i32,
    counts: Vec<Count>,
}

#[derive(Clone, Debug, Serialize, Deserialize, juniper::GraphQLObject)]
struct Count {
    #[serde(rename = "_id")]
    timestamp: i32,
    aggregate_count: i32,
    record_count: i32,
}

pub fn count_events_by_group(
    ctx: &Clients,
    application_id: &ID,
    window: &WindowType,
    start_timestamp: i32,
    end_timestamp: i32,
    grouping: &str,
    nested_grouping: &str,
) -> Result<CountResponse, FieldError> {
    if !is_valid_application(application_id) {
        return Err("Invalid application ID".into());
    }

    let collection_name = get_collection_name(application_id, Some(window));
    let service = &ctx.mongo.get_mongo_service(&collection_name).unwrap();
    let start_timestamp = get_timestamp_start(window, start_timestamp);
    let end_timestamp = get_timestamp_start(window, end_timestamp);

    let grouping = grouping.to_ascii_lowercase();
    let nested_grouping = vec![nested_grouping.to_ascii_lowercase()];
    let match_doc = doc! {
        "$match": {
            "grouping": grouping,
            "nested_grouping_ids": { "$in": nested_grouping },
            "timestamp": { "$gte": start_timestamp, "$lte": end_timestamp },
        },
    };
    let group_doc = doc! {
        "$group": {
            "_id": "$timestamp",
            "record_count": { "$sum": 1 },
            "aggregate_count": { "$sum": "$count" },
        }
    };
    let result = service
        .data_source()
        .aggregate(vec![match_doc.clone(), group_doc.clone()], None)?;

    let mut count_response = CountResponse {
        total_aggregate_count: 0,
        total_record_count: 0,
        counts: vec![],
    };

    result.for_each(|r| {
        if let Ok(doc) = r {
            let response: Count = bson::from_bson(bson::Bson::Document(doc.clone())).unwrap();
            count_response.total_aggregate_count += response.aggregate_count;
            count_response.total_record_count += response.record_count;
            count_response.counts.push(response);
        }
    });

    Ok(count_response)
}

pub fn query_event_groups(
    ctx: &Clients,
    application_id: &ID,
    window: &WindowType,
    start_timestamp: i32,
    end_timestamp: i32,
    grouping: &Option<String>,
    nested_grouping: &Option<String>,
) -> Result<FindResult<Bucket>, FieldError> {
    if !is_valid_application(application_id) {
        return Err("Invalid application ID".into());
    }

    let collection_name = get_collection_name(application_id, Some(window));

    let service = &ctx.mongo.get_mongo_service(&collection_name).unwrap();
    let start_timestamp = get_timestamp_start(window, start_timestamp);
    let end_timestamp = get_timestamp_start(window, end_timestamp);

    let mut filter = doc! {
        "timestamp": { "$gte": start_timestamp, "$lte": end_timestamp },
    };

    if let Some(grouping) = grouping {
        filter.insert("grouping", grouping.to_ascii_lowercase());
    }

    if let Some(nested_grouping) = nested_grouping {
        filter.insert(
            "nested_grouping_ids",
            vec![nested_grouping.to_ascii_lowercase()],
        );
    }

    let result: Result<FindResult<Bucket>, _> =
        service.find(Some(filter), None, None, None, None, None);
    result.map_err(|e| e.into())
}

#[derive(Serialize, Deserialize, juniper::GraphQLObject)]
pub struct LogEventResult {
    pub success: bool,
    pub inserted_id: Option<ID>,
}

/// TODO: This should just return back true, the storing of the event happens separately
///
/// However, it will check to see if the application_id is valid or not and return an error
pub fn log_event(
    ctx: &Clients,
    application_id: &ID,
    mut new_event: NewEvent,
    created_by_id: Option<ID>,
) -> Result<LogEventResult, FieldError> {
    if !is_valid_application(application_id) {
        return Err("Invalid application ID".into());
    }

    let application_id = lowercase_id(application_id);
    new_event.keys = new_event.keys.iter().map(|kp| kp.lowercase()).collect();

    let config = CONFIGS
        .read()
        .unwrap()
        .get(&application_id)
        .unwrap()
        .clone();

    // if we are logging all events then we'll have an inserted_id
    let log_all_events = config.log_all_events.unwrap_or(false);
    let inserted_id = if log_all_events {
        let collection_name = get_collection_name(&application_id, None);
        let service = &ctx.mongo.get_mongo_service(&collection_name).unwrap();
        let inserted_id: ID = service.insert_one(new_event.clone(), created_by_id)?;
        Some(inserted_id)
    } else {
        None
    };

    // create the embedded doc
    let mut embedded_doc = doc! {
        "timestamp": &new_event.timestamp,
        "raw_timestamp": now(),
    };
    new_event.keys.iter().for_each(|kp| {
        embedded_doc.insert(kp.key.clone(), kp.value.clone());
    });

    // keep going and put this into the various places it needs to go
    // loop through windows and groups
    // TODO: better error handling
    config.windows.iter().for_each(|window| {
        let collection_name = get_collection_name(&application_id, Some(&window));
        let service = &ctx.mongo.get_mongo_service(&collection_name).unwrap();

        // get all the groups
        config.groups.iter().for_each(|group| {
            // create a bucket object
            let timestamp = get_timestamp_start(window, new_event.timestamp);
            let hash = get_hash_id(window, group, &new_event.keys, timestamp);
            let id = ID::from_string(hash.clone());
            let nested_groupings = get_nested_groupings(&group, &config.groups);
            let nested_grouping_ids: Vec<String> = nested_groupings
                .iter()
                .map(|group_def| get_group_id(group_def, &new_event.keys))
                .collect();
            let mut update_doc = doc! {
                "$set": {
                    "application_id": application_id.to_bson(),
                    "grouping": group.clone(),
                    "grouping_id": get_group_id(&group, &new_event.keys),
                    "window": format!("{:?}", window),
                    "timestamp": timestamp,
                    "nested_grouping_ids": nested_grouping_ids,
                },
                "$inc": { "count": 1 },
                "$push": { "events": embedded_doc.clone() },
            };
            if let Some(inserted_id) = inserted_id.clone() {
                update_doc.insert(
                    "$push",
                    doc! {
                        "event_ids": inserted_id.to_bson(),
                        "events": embedded_doc.clone()
                    },
                );
            } else {
                update_doc.insert(
                    "$push",
                    doc! {
                        "events": embedded_doc.clone()
                    },
                );
            }
            let _result = service.data_source().update_one(
                doc! {
                    "_id": id.to_bson()
                },
                update_doc,
                Some(UpdateOptions {
                    upsert: Some(true),
                    ..UpdateOptions::default()
                }),
            );
        });
    });
    Ok(LogEventResult {
        success: true,
        inserted_id,
    })
}

/// looks in the all_groups and checks if the group starts with that (and is not the same)
///
/// For example:
/// ```rust
/// let group = "a|b|c|d";
/// let all_groups = vec!["a", "a|b", "a|c", "a|b|c", "b|c", "a|b|c|d", "a|b|c|d|e"];
///
/// // returns `["a", "a|b", "a|b|c"]`
/// ```
/// - Does not return a|c because out of order
/// - Does not return b|c because it's not at the start
/// - Does not return a|b|c|d because it's the same
/// - a|b|c|d|e is not a subset
fn get_nested_groupings(group: &str, all_groups: &Vec<String>) -> Vec<String> {
    all_groups.iter().fold(Vec::new(), |mut acc, test_group| {
        if group != test_group {
            if let Some(index) = group.find(test_group) {
                if index == 0 {
                    acc.push(test_group.clone());
                }
            }
        }
        acc
    })
}
