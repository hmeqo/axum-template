use serde::Deserialize;
use validator::Validate;

/// Common path parameter for auto-increment ID
#[derive(Deserialize, Validate)]
pub struct AutoIdPath {
    #[validate(range(min = 1))]
    pub id: i32,
}
