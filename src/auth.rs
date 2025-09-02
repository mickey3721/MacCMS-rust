use crate::models::User;
use mongodb::{bson::doc, Database};
use std::env;

// This function is called on startup to ensure the admin user exists.
pub async fn ensure_admin_user_exists(db: &Database) {
    let user_collection = db.collection::<User>("users");

    let admin_user = env::var("ADMIN_USER").expect("ADMIN_USER not set in .env");
    let admin_pass = env::var("ADMIN_PASS").expect("ADMIN_PASS not set in .env");

    match user_collection
        .find_one(doc! { "user_name": &admin_user }, None)
        .await
    {
        Ok(Some(_)) => {
            // Admin user already exists
            println!("Admin user '{}' already exists.", admin_user);
        }
        Ok(None) => {
            // Admin user does not exist, create it
            println!("Admin user '{}' not found, creating now...", admin_user);

            let hashed_password = match bcrypt::hash(&admin_pass, bcrypt::DEFAULT_COST) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("Failed to hash password: {}", e);
                    return;
                }
            };

            let new_admin = User {
                id: None,
                user_name: admin_user.clone(),
                user_pwd: hashed_password,
                group_id: 1, // Assuming 1 is the admin group
                user_status: 1,
                user_nick_name: Some(admin_user.clone()),
                user_email: None,
                user_phone: None,
                user_portrait: None,
                user_points: 0,
                user_end_time: mongodb::bson::DateTime::from_millis(253402300799999), // Using a large timestamp for "never expires"
                vip_level: None,
                vip_end_time: None,
                created_at: Some(mongodb::bson::DateTime::now()),
            };

            match user_collection.insert_one(new_admin, None).await {
                Ok(_) => println!("Successfully created admin user '{}'.", admin_user),
                Err(e) => eprintln!("Failed to create admin user: {}", e),
            }
        }
        Err(e) => {
            eprintln!("Failed to query for admin user: {}", e);
        }
    }
}
