mod models;
mod db;
mod dto;
mod api_handlers;
mod template;
mod web_handlers;
mod auth;
mod admin_handlers;
mod init_data;
mod collect_handlers;
mod index_manager;

use admin_handlers::{
    get_types, create_type, update_type, delete_type, 
    get_bindings, create_or_update_binding, get_collection_binding_status,
    get_configs, get_config_by_key, create_config, update_config, delete_config,
    get_collections, create_collection, update_collection, delete_collection,
    get_vods_admin, create_vod, update_vod, delete_vod,
    start_collection_collect, get_collect_progress, get_running_tasks,
    create_indexes, get_index_status, list_indexes, get_statistics
};
use collect_handlers::{
    get_collect_categories, get_collect_videos, start_collect_task
};

use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use mongodb::Database;
use futures::stream::TryStreamExt;
use actix_session::{SessionMiddleware, storage::CookieSessionStore};
use actix_web_flash_messages::{FlashMessagesFramework, storage::CookieMessageStore};
use actix_web::cookie::Key;
use actix_files::Files;
use std::env;

// Handler to get a list of vods
#[get("/vods")]
async fn get_vods(db: web::Data<Database>) -> impl Responder {
    let collection = db.collection::<models::Vod>("vods");

    match collection.find(None, None).await {
        Ok(cursor) => {
            let vods: Vec<models::Vod> = match cursor.try_collect().await {
                Ok(docs) => docs,
                Err(e) => {
                    eprintln!("Failed to collect documents: {}", e);
                    return HttpResponse::InternalServerError().body("Failed to collect documents");
                }
            };
            HttpResponse::Ok().json(vods)
        }
        Err(e) => {
            eprintln!("Failed to execute find: {}", e);
            HttpResponse::InternalServerError().body("Failed to fetch videos")
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize the database
    let db = match db::init().await {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to connect to the database: {}", e);
            // Exit the application if the database connection fails
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "DB connection failed"));
        }
    };
    
    println!("Database connection successful!");
    
    // 初始化数据库索引
    let index_manager = index_manager::IndexManager::new(db.clone());
    println!("🔧 正在检查和创建数据库索引...");
    match index_manager.create_all_indexes().await {
        Ok(_) => {
            println!("✅ 数据库索引初始化完成");
        }
        Err(e) => {
            eprintln!("⚠️  索引创建过程中出现警告: {}", e);
            // 不退出应用，因为基本功能仍可使用
        }
    }
    
    auth::ensure_admin_user_exists(&db).await;
    
    // 初始化测试数据
    println!("🔧 正在初始化测试数据...");
    match init_data::init_all_data(&db).await {
        Ok(_) => {
            println!("✅ 测试数据初始化完成");
        }
        Err(e) => {
            eprintln!("⚠️  测试数据初始化失败: {}", e);
            // 不退出应用，因为基本功能仍可使用
        }
    }

    let session_secret_key = Key::generate();
    
    println!("Starting server at http://127.0.0.1:8080");

    HttpServer::new(move || {
        App::new()
            // Store the database connection in the application state
            .app_data(web::Data::new(db.clone()))
            // Session and Flash Messages Middleware
            .wrap(FlashMessagesFramework::builder(
                CookieMessageStore::builder(Key::generate()).build()
            ).build())
            
            .wrap(SessionMiddleware::builder(CookieSessionStore::default(), session_secret_key.clone()).build())
            // Web routes
            .service(web::resource("/").route(web::get().to(web_handlers::home_page)))
            .service(web::resource("/list/{type_id}").route(web::get().to(web_handlers::list_page_handler)))
            .service(web::resource("/detail/{vod_id}").route(web::get().to(web_handlers::video_detail_handler)))
            .service(web::resource("/play/{vod_id}/{play_index}").route(web::get().to(web_handlers::video_player_handler)))
            .service(web::resource("/search").route(web::get().to(web_handlers::search_page_handler)))
            // Static files
            .service(Files::new("/static", "./static").show_files_listing())
            // Admin Web routes
            .service(web::resource("/admin/login")
                .route(web::get().to(web_handlers::login_page))
                .route(web::post().to(web_handlers::login_post)))
            .service(web::resource("/admin").route(web::get().to(web_handlers::admin_dashboard)))
            .service(web::resource("/admin/logout").route(web::get().to(web_handlers::logout)))
            .service(web::resource("/admin/types").route(web::get().to(web_handlers::admin_types_page)))
            .service(web::resource("/admin/vods").route(web::get().to(web_handlers::admin_vods_page)))
            .service(web::resource("/admin/collect").route(web::get().to(web_handlers::admin_collect_page)))
            .service(web::resource("/admin/collect_vod").route(web::get().to(web_handlers::admin_collect_vod_page)))
            .service(web::resource("/admin/bindings").route(web::get().to(web_handlers::admin_bindings_page)))
            .service(web::resource("/admin/config").route(web::get().to(web_handlers::admin_config_page)))
            .service(web::resource("/admin/indexes").route(web::get().to(web_handlers::admin_indexes_page)))
            .service(web::resource("/admin/init-data").route(web::post().to(web_handlers::init_data_handler)))
            // API routes
            .service(get_vods)
            .service(web::resource("/api/provide/vod").route(web::get().to(api_handlers::provide_vod)))
            .service(web::resource("/api/videos/{type_id}").route(web::get().to(api_handlers::get_videos_by_type)))
            .service(web::resource("/api/categories/hierarchy").route(web::get().to(api_handlers::get_category_hierarchy)))
            .service(web::resource("/api/videos/detail/{vod_id}").route(web::get().to(api_handlers::get_video_details)))
            .service(web::resource("/api/filter-options").route(web::get().to(api_handlers::get_filter_options)))
            // Admin API routes
            .service(
                web::scope("/api/admin")
                    // Category Management
                    .service(web::resource("/types")
                        .route(web::get().to(get_types))
                        .route(web::post().to(create_type)))
                    .service(web::resource("/types/{id}")
                        .route(web::put().to(update_type))
                        .route(web::delete().to(delete_type)))
                    // Binding Management
                    .service(web::resource("/bindings")
                        .route(web::get().to(get_bindings))
                        .route(web::post().to(create_or_update_binding)))
                    // Website Configuration
                    .service(web::resource("/configs")
                        .route(web::get().to(get_configs))
                        .route(web::post().to(create_config)))
                    .service(web::resource("/configs/{key}")
                        .route(web::get().to(get_config_by_key))
                        .route(web::put().to(update_config))
                        .route(web::delete().to(delete_config)))
                    // Collection Management
                    .service(web::resource("/collections")
                        .route(web::get().to(get_collections))
                        .route(web::post().to(create_collection)))
                    .service(web::resource("/collections/{id}")
                        .route(web::put().to(update_collection))
                        .route(web::delete().to(delete_collection)))
                    .service(web::resource("/collections/{id}/binding-status")
                        .route(web::get().to(get_collection_binding_status)))
                    .service(web::resource("/collections/{id}/collect")
                        .route(web::post().to(start_collection_collect)))
                    .service(web::resource("/collect/progress/{task_id}")
                        .route(web::get().to(get_collect_progress)))
                    .service(web::resource("/running-tasks")
                        .route(web::get().to(get_running_tasks)))
                    // Video Management
                    .service(web::resource("/vods")
                        .route(web::get().to(get_vods_admin))
                        .route(web::post().to(create_vod)))
                    .service(web::resource("/vods/{id}")
                        .route(web::put().to(update_vod))
                        .route(web::delete().to(delete_vod)))
                    // Index Management
                    .service(web::resource("/indexes/create")
                        .route(web::post().to(create_indexes)))
                    .service(web::resource("/indexes/status")
                        .route(web::get().to(get_index_status)))
                    .service(web::resource("/indexes/list")
                        .route(web::get().to(list_indexes)))
                    // Statistics
                    .service(web::resource("/statistics")
                        .route(web::get().to(get_statistics)))
            )
            // Collect API routes
            .service(
                web::scope("/api/collect")
                    .service(web::resource("/categories").route(web::get().to(get_collect_categories)))
                    .service(web::resource("/videos").route(web::get().to(get_collect_videos)))
                    .service(web::resource("/start").route(web::post().to(start_collect_task)))
                    .service(web::resource("/progress/{task_id}").route(web::get().to(get_collect_progress)))
            )
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}





