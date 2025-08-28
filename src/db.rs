use dotenv::dotenv;
use mongodb::{Client, Database};
use std::env;

pub async fn init() -> Result<Database, mongodb::error::Error> {
    dotenv().ok(); // This line loads the .env file
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let client = Client::with_uri_str(&database_url).await?;
    let database_name = env::var("DATABASE_NAME").expect("DATABASE_NAME must be set");
    Ok(client.database(&database_name))
}
