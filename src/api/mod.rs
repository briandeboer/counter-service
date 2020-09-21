pub mod events;

use mongodb_base_service::ID;

pub fn lowercase_id(id: &ID) -> ID {
    ID::from(id.to_string().to_ascii_lowercase())
}
