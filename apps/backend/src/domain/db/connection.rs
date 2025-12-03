use sea_orm::{Database, DatabaseConnection};

/// Establish a database connection
pub async fn init_db(url: &str) -> Result<DatabaseConnection, sea_orm::DbErr> {
    Database::connect(url).await
}
