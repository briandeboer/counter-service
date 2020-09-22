use crate::api;
use crate::api::events::LogEventResult;
use crate::db::Clients;
use crate::models::NewEvent;

use actix_web::{error::ErrorUnauthorized, web, Error};
use jwt_validator::Claims;
use log::error;
use mongodb_base_service::ID;
use percent_encoding::percent_decode_str;
use std::env;
use std::sync::Arc;

lazy_static! {
    static ref DISABLE_AUTH: u8 = env::var("DISABLE_AUTH")
        .unwrap_or("".to_string())
        .parse()
        .unwrap_or(0);
}

fn get_unencoded_value(value: &str) -> String {
    percent_decode_str(value).decode_utf8().unwrap().to_string()
}

pub async fn log_events(
    ctx: web::Data<Arc<Clients>>,
    application_id: web::Path<String>,
    events: web::Json<Vec<NewEvent>>,
    claims: Option<Claims>,
) -> Result<web::Json<Vec<LogEventResult>>, Error> {
    if *DISABLE_AUTH != 1 && claims.is_none() {
        return Err(ErrorUnauthorized("Invalid request"));
    }

    let application_id = ID::from_string(get_unencoded_value(&application_id));

    if !api::events::is_valid_application(&application_id) {
        return Err(ErrorUnauthorized("Invalid applicationId"));
    }

    let mut results = vec![];
    events.iter().for_each(|new_event| {
        match api::events::log_event(ctx.get_ref(), &application_id, new_event.clone(), None) {
            Ok(result) => {
                results.push(result);
            }
            Err(e) => {
                error!("Error occurred logginng event {:?}", e);
                results.push(LogEventResult {
                    success: false,
                    inserted_id: None,
                });
            }
        }
    });

    Ok(web::Json(results))
}
