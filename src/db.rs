use mongodb::{Client, Database, options::ClientOptions};
use std::env;
use std::time::Duration;
use dotenv::dotenv;

pub async fn init() -> Result<Database, mongodb::error::Error> {
    dotenv().ok(); // This line loads the .env file
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    
    // 配置连接池选项
    let mut options = ClientOptions::parse(&database_url).await?;
    options.max_pool_size = Some(20);
    options.min_pool_size = Some(5);
    options.max_idle_time = Some(Duration::from_secs(30));
    
    let client = Client::with_options(options)?;
    let database_name = env::var("DATABASE_NAME").expect("DATABASE_NAME must be set");
    Ok(client.database(&database_name))
}
