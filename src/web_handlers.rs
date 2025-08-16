use actix_web::{web, HttpResponse, Responder};
use mongodb::{Database, bson::doc, options::FindOptions};
use crate::template::TERA;
use crate::models::{Type, Vod, User};
use futures::stream::TryStreamExt;
use serde::{Serialize, Deserialize};
use crate::dto::ListPageParams;
use actix_session::Session;
use actix_web_flash_messages::FlashMessage;
use crate::init_data;
use std::error::Error;

#[derive(Serialize)]
struct CategorizedVideos {
    category: Type,
    videos: Vec<Vod>,
}

// --- Frontend Web Handlers ---

pub async fn home_page(db: web::Data<Database>) -> impl Responder {
    let mut context = tera::Context::new();

    // 1. Fetch all categories for navigation
    let type_collection = db.collection::<Type>("types");
    let nav_categories: Vec<Type> = match type_collection.find(doc! { "type_pid": 0 }, None).await {
        Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
        Err(_) => vec![],
    };
    
    // Fetch sub-categories for navigation menu
    let mut categories_with_subs = Vec::new();
    for category in &nav_categories {
        let sub_categories: Vec<Type> = match type_collection.find(doc! { "type_pid": category.type_id }, None).await {
            Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
            Err(_) => vec![],
        };
        categories_with_subs.push((category.clone(), sub_categories));
    }
    
    context.insert("categories", &nav_categories);
    context.insert("categories_with_subs", &categories_with_subs);

    // 2. Fetch videos for each top-level category (include sub-categories)
    let vod_collection = db.collection::<Vod>("vods");
    let mut categorized_videos_list = Vec::new();

    for category in nav_categories.iter() { // Iterate over references
        let find_options = FindOptions::builder()
            .sort(doc! { "vod_pubdate": -1 })
            .limit(12)
            .build();
        
        // Get all sub-category IDs for this top-level category
        let sub_categories: Vec<Type> = match type_collection.find(doc! { "type_pid": category.type_id }, None).await {
            Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
            Err(_) => vec![],
        };
        
        // Build filter to include both top-level category and its sub-categories
        let mut type_ids = vec![category.type_id];
        for sub_cat in &sub_categories {
            type_ids.push(sub_cat.type_id);
        }
        
        let videos = match vod_collection.find(doc! { "type_id": { "$in": type_ids } }, find_options).await {
            Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
            Err(_) => vec![],
        };

        categorized_videos_list.push(CategorizedVideos {
            category: category.clone(), // Clone the category for the struct
            videos,
        });
    }

    context.insert("categorized_videos", &categorized_videos_list);
    context.insert("SITENAME", "maccms-rust");

    // 3. Render the template
    match TERA.render("index.html", &context) {
        Ok(s) => {
            println!("Debug: Template rendered successfully, length: {}", s.len());
            HttpResponse::Ok().content_type("text/html").body(s)
        },
        Err(e) => {
            println!("Template rendering error: {}", e);
            println!("Error details: {:?}", e);
            HttpResponse::InternalServerError().body(format!("Template error: {}", e))
        }
    }
}

// Video detail page handler
pub async fn video_detail_handler(
    path: web::Path<String>,
    db: web::Data<Database>,
) -> impl Responder {
    let vod_id = path.into_inner();
    let mut context = tera::Context::new();

    let vod_collection = db.collection::<Vod>("vods");
    let type_collection = db.collection::<Type>("types");

    // Parse ObjectId from string
    let object_id = match mongodb::bson::oid::ObjectId::parse_str(&vod_id) {
        Ok(id) => id,
        Err(_) => return HttpResponse::NotFound().body("Invalid video ID"),
    };

    // Add navigation data
    let nav_categories: Vec<Type> = match type_collection.find(doc! { "type_pid": 0 }, None).await {
        Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
        Err(_) => vec![],
    };
    
    // Fetch sub-categories for navigation menu
    let mut categories_with_subs = Vec::new();
    for category in &nav_categories {
        let sub_categories: Vec<Type> = match type_collection.find(doc! { "type_pid": category.type_id }, None).await {
            Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
            Err(_) => vec![],
        };
        categories_with_subs.push((category.clone(), sub_categories));
    }
    
    context.insert("categories", &nav_categories);
    context.insert("categories_with_subs", &categories_with_subs);

    // 1. Fetch video details
    let video = match vod_collection.find_one(doc!{"_id": object_id}, None).await {
        Ok(Some(v)) => v,
        _ => return HttpResponse::NotFound().body("Video not found"),
    };
    
    // Convert MongoDB DateTime to timestamp for template
    let pubdate_timestamp = video.vod_pubdate.timestamp_millis() / 1000;
    context.insert("vod_pubdate_timestamp", &pubdate_timestamp);
    context.insert("video", &video);

    // 2. Fetch category info
    if let Ok(Some(category)) = type_collection.find_one(doc!{"type_id": video.type_id}, None).await {
        context.insert("category", &category);
    }

    // 3. Fetch related videos (same category)
    let find_options = FindOptions::builder()
        .sort(doc! { "vod_pubdate": -1 })
        .limit(10)
        .build();
    
    let related_videos: Vec<Vod> = match vod_collection.find(
        doc! { "type_id": video.type_id, "_id": { "$ne": object_id } }, 
        find_options
    ).await {
        Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
        Err(_) => vec![],
    };
    
    // Convert related videos dates to timestamps
    let related_timestamps: Vec<i64> = related_videos.iter()
        .map(|v| v.vod_pubdate.timestamp_millis() / 1000)
        .collect();
    context.insert("related_videos", &related_videos);
    context.insert("related_pubdate_timestamps", &related_timestamps);

    context.insert("SITENAME", "maccms-rust");

    // 4. Render the template
    match TERA.render("detail.html", &context) {
        Ok(s) => HttpResponse::Ok().content_type("text/html").body(s),
        Err(e) => {
            println!("Template rendering error: {}", e);
            println!("Error kind: {:?}", e.kind);
            println!("Error source: {:?}", e.source());
            HttpResponse::InternalServerError().body(format!("Template error: {}", e))
        }
    }
}

// Video player page handler
pub async fn video_player_handler(
    path: web::Path<(String, String)>,
    db: web::Data<Database>,
) -> impl Responder {
    let (vod_id, play_index) = path.into_inner();
    let mut context = tera::Context::new();

    let vod_collection = db.collection::<Vod>("vods");
    let type_collection = db.collection::<Type>("types");

    // Parse ObjectId from string
    let object_id = match mongodb::bson::oid::ObjectId::parse_str(&vod_id) {
        Ok(id) => id,
        Err(_) => return HttpResponse::NotFound().body("Invalid video ID"),
    };

    // Add navigation data
    let nav_categories: Vec<Type> = match type_collection.find(doc! { "type_pid": 0 }, None).await {
        Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
        Err(_) => vec![],
    };
    
    // Fetch sub-categories for navigation menu
    let mut categories_with_subs = Vec::new();
    for category in &nav_categories {
        let sub_categories: Vec<Type> = match type_collection.find(doc! { "type_pid": category.type_id }, None).await {
            Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
            Err(_) => vec![],
        };
        categories_with_subs.push((category.clone(), sub_categories));
    }
    
    context.insert("categories", &nav_categories);
    context.insert("categories_with_subs", &categories_with_subs);

    // Parse play index (format: "source-index" or "index")
    let play_idx: usize = if play_index.contains('-') {
        // Format: "source-index", extract the index part
        match play_index.split('-').last() {
            Some(idx_str) => match idx_str.parse() {
                Ok(idx) => idx,
                Err(_) => return HttpResponse::NotFound().body("Invalid play index format"),
            },
            None => return HttpResponse::NotFound().body("Invalid play index format"),
        }
    } else {
        // Format: "index"
        match play_index.parse() {
            Ok(idx) => idx,
            Err(_) => return HttpResponse::NotFound().body("Invalid play index"),
        }
    };

    // 1. Fetch video details and increment hit count
    let video = match vod_collection.find_one(doc!{"_id": object_id}, None).await {
        Ok(Some(v)) => v,
        _ => return HttpResponse::NotFound().body("Video not found"),
    };
    
    // Increment hit count
    let current_hits = video.vod_hits.unwrap_or(0);
    let current_hits_day = video.vod_hits_day.unwrap_or(0);
    let current_hits_week = video.vod_hits_week.unwrap_or(0);
    let current_hits_month = video.vod_hits_month.unwrap_or(0);
    
    let update_result = vod_collection.update_one(
        doc! {"_id": object_id},
        doc! {"$set": {
            "vod_hits": current_hits + 1,
            "vod_hits_day": current_hits_day + 1,
            "vod_hits_week": current_hits_week + 1,
            "vod_hits_month": current_hits_month + 1,
        }},
        None
    ).await;
    
    if let Err(e) = update_result {
        println!("Warning: Failed to update hit count: {}", e);
    }
    
    // Convert MongoDB DateTime to timestamp for template
    let pubdate_timestamp = video.vod_pubdate.timestamp_millis() / 1000;
    context.insert("vod_pubdate_timestamp", &pubdate_timestamp);
    context.insert("video", &video);

    // 2. Get play URL
    let play_url = if let Some(source) = video.vod_play_urls.get(0) {
        if let Some(url_info) = source.urls.get(play_idx) {
            url_info.url.clone()
        } else {
            return HttpResponse::NotFound().body("Play URL not found");
        }
    } else {
        return HttpResponse::NotFound().body("No play sources available");
    };

    context.insert("play_url", &play_url);
    context.insert("play_index", &play_idx);
    context.insert("SITENAME", "maccms-rust");

    // 3. Render the template
    match TERA.render("player.html", &context) {
        Ok(s) => HttpResponse::Ok().content_type("text/html").body(s),
        Err(e) => {
            println!("Template rendering error: {}", e);
            println!("Error kind: {:?}", e.kind);
            println!("Error source: {:?}", e.source());
            
            // Print more details about the context
            println!("Context contains keys (but we can't iterate over them directly)");
            
            HttpResponse::InternalServerError().body(format!("Template error: {} - Kind: {:?}", e, e.kind))
        }
    }
}

#[derive(Serialize)]
struct PaginationInfo {
    current_page: u64,
    total_pages: u64,
    total_items: u64,
    pages: Vec<u64>,
}

pub async fn list_page_handler(
    path: web::Path<i32>,
    query: web::Query<ListPageParams>,
    db: web::Data<Database>,
) -> impl Responder {
    let type_id = path.into_inner();
    let mut context = tera::Context::new();

    // Add navigation data
    let type_collection = db.collection::<Type>("types");
    let nav_categories: Vec<Type> = match type_collection.find(doc! { "type_pid": 0 }, None).await {
        Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
        Err(_) => vec![],
    };
    
    // Fetch sub-categories for navigation menu
    let mut categories_with_subs = Vec::new();
    for category in &nav_categories {
        let sub_categories: Vec<Type> = match type_collection.find(doc! { "type_pid": category.type_id }, None).await {
            Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
            Err(_) => vec![],
        };
        categories_with_subs.push((category.clone(), sub_categories));
    }
    
    context.insert("categories", &nav_categories);
    context.insert("categories_with_subs", &categories_with_subs);

    // Initialize filter variables for template
    context.insert("current_sub_type", &None::<i32>);
    context.insert("current_area", &None::<String>);
    context.insert("current_year", &None::<String>);
    context.insert("current_sort", &query.sort);

    let vod_collection = db.collection::<Vod>("vods");

    // 1. Fetch main category info
    let main_category = match type_collection.find_one(doc!{"type_id": type_id}, None).await {
        Ok(Some(cat)) => cat,
        _ => return HttpResponse::NotFound().body("Category not found"),
    };
    context.insert("category", &main_category);

    // 2. Parse subarea and subyear for filtering options
    let subarea_options: Vec<String> = if let Some(subarea) = &main_category.subarea {
        subarea.split(',').map(|s| s.trim().to_string()).collect()
    } else {
        vec![]
    };
    let subyear_options: Vec<String> = if let Some(subyear) = &main_category.subyear {
        subyear.split(',').map(|s| s.trim().to_string()).collect()
    } else {
        vec![]
    };
    context.insert("subarea_options", &subarea_options);
    context.insert("subyear_options", &subyear_options);

    // 3. Fetch sub-categories for filter
    let sub_categories: Vec<Type> = match type_collection.find(doc!{"type_pid": type_id}, None).await {
        Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
        Err(_) => vec![],
    };
    context.insert("sub_categories", &sub_categories);

    // 4. Build filter for videos
    let mut filter = doc! {};
    
    // Handle sub_type filtering - if sub_type is provided, use it instead of main type_id
    if let Some(sub_type) = query.sub_type {
        context.insert("current_sub_type", &sub_type);
        filter.insert("type_id", sub_type);
    } else {
        // If no sub_type is selected, include main category and all its sub-categories
        let mut type_ids = vec![type_id];
        for sub_cat in &sub_categories {
            type_ids.push(sub_cat.type_id);
        }
        filter.insert("type_id", doc! { "$in": type_ids });
    }

    if let Some(area) = &query.area {
        filter.insert("vod_area", area);
        context.insert("current_area", area);
    }
    if let Some(year) = &query.year {
        filter.insert("vod_year", year);
        context.insert("current_year", year);
    }

    // 5. Pagination setup
    let page = query.pg.unwrap_or(1);
    let limit = 20; // Items per page
    let skip = if page > 0 { (page - 1) * limit } else { 0 };

    // 6. Count total documents for pagination
    let total_items = match vod_collection.count_documents(filter.clone(), None).await {
        Ok(count) => count,
        Err(_) => 0,
    };
    
    let total_pages = if total_items > 0 {
        (total_items as f64 / limit as f64).ceil() as u64
    } else {
        0
    };

    // 7. Build sort options based on query parameter
    let sort_doc = match query.sort.as_deref() {
        Some("hits") => doc! { "vod_hits": -1 }, // Most played
        Some("score") => doc! { "vod_score": -1 }, // Highest rated
        Some("year_desc") => doc! { "vod_year": -1 }, // Newest year
        Some("year_asc") => doc! { "vod_year": 1 }, // Oldest year
        Some("name_asc") => doc! { "vod_name": 1 }, // Name A-Z
        Some("name_desc") => doc! { "vod_name": -1 }, // Name Z-A
        _ => doc! { "vod_pubdate": -1 }, // Default: latest published
    };

    // 8. Fetch videos based on filter with pagination
    let find_options = FindOptions::builder()
        .skip(Some(skip as u64))
        .limit(Some(limit as i64))
        .sort(sort_doc)
        .build();

    let vods: Vec<Vod> = match vod_collection.find(filter, find_options).await {
        Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
        Err(_) => vec![],
    };
    context.insert("vods", &vods);

    // 9. Add pagination info to context
    if total_pages > 1 {
        let mut pages = Vec::new();
        let start_page = if page > 3 { page - 3 } else { 1 };
        let end_page = if page + 3 < total_pages { page + 3 } else { total_pages };
        
        for p in start_page..=end_page {
            pages.push(p);
        }

        let pagination = PaginationInfo {
            current_page: page,
            total_pages,
            total_items,
            pages,
        };
        context.insert("pagination", &pagination);
    }

    // 10. Add common variables
    context.insert("SITENAME", "maccms-rust");

    // 11. Render the template
    match TERA.render("list.html", &context) {
        Ok(s) => HttpResponse::Ok().content_type("text/html").body(s),
        Err(e) => {
            println!("Template rendering error: {}", e);
            println!("Error details: {:?}", e);
            HttpResponse::InternalServerError().body(format!("Template error: {}", e))
        }
    }
}

// Search page handler
pub async fn search_page_handler(
    query: web::Query<crate::dto::ApiParams>,
    db: web::Data<Database>,
) -> impl Responder {
    let mut context = tera::Context::new();

    // Add navigation data
    let type_collection = db.collection::<Type>("types");
    let nav_categories: Vec<Type> = match type_collection.find(doc! { "type_pid": 0 }, None).await {
        Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
        Err(_) => vec![],
    };
    
    // Fetch sub-categories for navigation menu
    let mut categories_with_subs = Vec::new();
    for category in &nav_categories {
        let sub_categories: Vec<Type> = match type_collection.find(doc! { "type_pid": category.type_id }, None).await {
            Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
            Err(_) => vec![],
        };
        categories_with_subs.push((category.clone(), sub_categories));
    }
    
    context.insert("categories", &nav_categories);
    context.insert("categories_with_subs", &categories_with_subs);

    let vod_collection = db.collection::<Vod>("vods");
    let search_results = if let Some(ref keyword) = query.wd {
        let search_filter = doc! {
            "$or": [
                { "vod_name": doc! { "$regex": keyword, "$options": "i" } },
                { "vod_actor": doc! { "$regex": keyword, "$options": "i" } },
                { "vod_director": doc! { "$regex": keyword, "$options": "i" } }
            ]
        };
        
        let find_options = FindOptions::builder()
            .sort(doc! { "vod_pubdate": -1 })
            .limit(50)
            .build();
        
        match vod_collection.find(search_filter, find_options).await {
            Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
            Err(_) => vec![],
        }
    } else {
        vec![]
    };

    context.insert("search_results", &search_results);
    context.insert("search_keyword", &query.wd);
    context.insert("SITENAME", "maccms-rust");

    match TERA.render("search.html", &context) {
        Ok(s) => HttpResponse::Ok().content_type("text/html").body(s),
        Err(e) => {
            println!("Template rendering error: {}", e);
            println!("Error details: {:?}", e);
            HttpResponse::InternalServerError().body(format!("Template error: {}", e))
        }
    }
}

// --- Admin Web Handlers ---

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    username: String,
    password: String,
}



pub async fn login_page() -> impl Responder {
    let context = tera::Context::new();
    match TERA.render("admin/login.html", &context) {
        Ok(s) => HttpResponse::Ok().content_type("text/html").body(s),
        Err(e) => {
            eprintln!("[ERROR] Failed to render 'admin/login.html': {}", e);
            eprintln!("[ERROR] Error kind: {:?}", e.kind);
            eprintln!("[ERROR] Full error chain: {:?}", e);
            HttpResponse::InternalServerError().body("Template error")
        }
    }
}

pub async fn login_post(db: web::Data<Database>, form: web::Form<LoginForm>, session: Session) -> impl Responder {
    println!("[DEBUG] Login attempt - Username: '{}', Password length: {}", form.username, form.password.len());
    
    let user_collection = db.collection::<User>("users");

    let user = match user_collection.find_one(doc!{"user_name": &form.username}, None).await {
        Ok(Some(u)) => {
            println!("[DEBUG] User found in database: {}", u.user_name);
            u
        },
        Ok(None) => {
            println!("[DEBUG] User not found in database for username: {}", form.username);
            FlashMessage::error("Invalid username or password.").send();
            return HttpResponse::Found().append_header(("Location", "/admin/login")).finish();
        },
        Err(e) => {
            println!("[DEBUG] Database error when finding user: {}", e);
            FlashMessage::error("Invalid username or password.").send();
            return HttpResponse::Found().append_header(("Location", "/admin/login")).finish();
        }
    };

    println!("[DEBUG] Stored password hash: {}", user.user_pwd);
    let password_valid = bcrypt::verify(&form.password, &user.user_pwd).unwrap_or(false);
    println!("[DEBUG] Password verification result: {}", password_valid);

    if password_valid {
        let user_id_str = user.id.unwrap().to_string();
        println!("[DEBUG] Setting session user_id: {}", user_id_str);
        
        match session.insert("user_id", user_id_str) {
            Ok(_) => {
                println!("[DEBUG] Session set successfully, redirecting to /admin");
                HttpResponse::Found().append_header(("Location", "/admin")).finish()
            },
            Err(e) => {
                println!("[DEBUG] Failed to set session: {}", e);
                FlashMessage::error("Login failed due to session error.").send();
                HttpResponse::Found().append_header(("Location", "/admin/login")).finish()
            }
        }
    } else {
        println!("[DEBUG] Password verification failed, redirecting back to login");
        FlashMessage::error("Invalid username or password.").send();
        HttpResponse::Found().append_header(("Location", "/admin/login")).finish()
    }
}

pub async fn admin_dashboard(session: Session, db: web::Data<Database>) -> impl Responder {
    if session.get::<String>("user_id").ok().flatten().is_none() {
        return HttpResponse::Found().append_header(("Location", "/admin/login")).finish();
    }

    let mut context = tera::Context::new();
    context.insert("SITENAME", "maccms-rust");
    
    // 获取统计数据
    let mut total_videos = 0;
    let mut total_categories = 0;
    let mut total_collections = 0;
    let mut total_configs = 0;
    let mut total_bindings = 0;
    let mut total_users = 0;
    
    // 获取视频数量
    if let Ok(count) = db.collection::<mongodb::bson::Document>("vods")
        .count_documents(None, None).await {
        total_videos = count as i32;
    }
    
    // 获取分类数量
    if let Ok(count) = db.collection::<mongodb::bson::Document>("types")
        .count_documents(None, None).await {
        total_categories = count as i32;
    }
    
    // 获取采集源数量
    if let Ok(count) = db.collection::<mongodb::bson::Document>("collections")
        .count_documents(None, None).await {
        total_collections = count as i32;
    }
    
    // 获取配置数量
    if let Ok(count) = db.collection::<mongodb::bson::Document>("configs")
        .count_documents(None, None).await {
        total_configs = count as i32;
    }
    
    // 获取绑定数量
    if let Ok(count) = db.collection::<mongodb::bson::Document>("bindings")
        .count_documents(None, None).await {
        total_bindings = count as i32;
    }
    
    // 获取用户数量
    if let Ok(count) = db.collection::<mongodb::bson::Document>("users")
        .count_documents(None, None).await {
        total_users = count as i32;
    }
    
    // 插入统计数据到模板上下文
    context.insert("total_videos", &total_videos);
    context.insert("total_categories", &total_categories);
    context.insert("total_collections", &total_collections);
    context.insert("total_configs", &total_configs);
    context.insert("total_bindings", &total_bindings);
    context.insert("total_users", &total_users);
    
    match TERA.render("admin/index.html", &context) {
        Ok(s) => HttpResponse::Ok().content_type("text/html").body(s),
        Err(e) => {
            eprintln!("[ERROR] Failed to render 'admin/index.html': {}", e);
            eprintln!("[ERROR] Error kind: {:?}", e.kind);
            eprintln!("[ERROR] Full error chain: {:?}", e);
            HttpResponse::InternalServerError().body("Template error")
        }
    }
}

pub async fn logout(session: Session) -> impl Responder {
    session.clear();
    HttpResponse::Found().append_header(("Location", "/admin/login")).finish()
}

pub async fn admin_types_page(session: Session, db: web::Data<Database>) -> impl Responder {
    if session.get::<String>("user_id").ok().flatten().is_none() {
        return HttpResponse::Found().append_header(("Location", "/admin/login")).finish();
    }

    let type_collection = db.collection::<Type>("types");
    let types: Vec<Type> = match type_collection.find(None, None).await {
        Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
        Err(e) => {
            eprintln!("Failed to fetch types: {}", e);
            vec![]
        }
    };

    let mut context = tera::Context::new();
    context.insert("SITENAME", "maccms-rust");
    context.insert("types", &types);

    match TERA.render("admin/types.html", &context) {
        Ok(s) => HttpResponse::Ok().content_type("text/html").body(s),
        Err(e) => {
            eprintln!("[ERROR] Failed to render 'admin/types.html': {}", e);
            eprintln!("[ERROR] Error kind: {:?}", e.kind);
            eprintln!("[ERROR] Full error chain: {:?}", e);
            HttpResponse::InternalServerError().body("Template error")
        }
    }
}

pub async fn init_data_handler(session: Session, db: web::Data<Database>) -> impl Responder {
    // Check if user is logged in
    if session.get::<String>("user_id").ok().flatten().is_none() {
        return HttpResponse::Found().append_header(("Location", "/admin/login")).finish();
    }

    match init_data::init_all_data(&db).await {
        Ok(_) => {
            FlashMessage::info("数据初始化成功！").send();
            HttpResponse::Found().append_header(("Location", "/admin")).finish()
        }
        Err(e) => {
            eprintln!("Data initialization failed: {}", e);
            FlashMessage::error(&format!("数据初始化失败: {}", e)).send();
            HttpResponse::Found().append_header(("Location", "/admin")).finish()
        }
    }
}



pub async fn admin_vods_page(session: Session, db: web::Data<Database>) -> impl Responder {
    if session.get::<String>("user_id").ok().flatten().is_none() {
        return HttpResponse::Found().append_header(("Location", "/admin/login")).finish();
    }

    let vod_collection = db.collection::<Vod>("vods");
    let find_options = FindOptions::builder().sort(doc!{"vod_pubdate": -1}).limit(50).build();
    let vods: Vec<Vod> = match vod_collection.find(None, find_options).await {
        Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
        Err(e) => {
            eprintln!("Failed to fetch vods: {}", e);
            vec![]
        }
    };

    let mut context = tera::Context::new();
    context.insert("SITENAME", "maccms-rust");
    context.insert("vods", &vods);

    match TERA.render("admin/vods.html", &context) {
        Ok(s) => HttpResponse::Ok().content_type("text/html").body(s),
        Err(e) => {
            eprintln!("[ERROR] Failed to render 'admin/vods.html': {}", e);
            eprintln!("[ERROR] Error kind: {:?}", e.kind);
            eprintln!("[ERROR] Full error chain: {:?}", e);
            HttpResponse::InternalServerError().body("Template error")
        }
    }
}

pub async fn admin_collect_page(session: Session, db: web::Data<Database>) -> impl Responder {
    if session.get::<String>("user_id").ok().flatten().is_none() {
        return HttpResponse::Found().append_header(("Location", "/admin/login")).finish();
    }
    let collection_collection = db.collection::<crate::models::Collection>("collections");
    let collections: Vec<crate::models::Collection> = match collection_collection.find(None, None).await {
        Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
        Err(e) => {
            eprintln!("Failed to fetch collections: {}", e);
            vec![]
        }
    };

    let mut context = tera::Context::new();
    context.insert("SITENAME", "maccms-rust");
    context.insert("collections", &collections);

    // println!("collections: {:?}", collections);

    match TERA.render("admin/collect.html", &context) {
        Ok(s) => HttpResponse::Ok().content_type("text/html").body(s),
        Err(e) => {
            eprintln!("[ERROR] Failed to render 'admin/collect.html': {}", e);
            eprintln!("[ERROR] Error kind: {:?}", e.kind);
            eprintln!("[ERROR] Full error chain: {:?}", e);
            HttpResponse::InternalServerError().body("Template error")
        }
    }
}

pub async fn admin_bindings_page(session: Session, db: web::Data<Database>) -> impl Responder {
    if session.get::<String>("user_id").ok().flatten().is_none() {
        return HttpResponse::Found().append_header(("Location", "/admin/login")).finish();
    }

    let binding_collection = db.collection::<crate::models::Binding>("bindings");
    let bindings: Vec<crate::models::Binding> = match binding_collection.find(None, None).await {
        Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
        Err(e) => {
            eprintln!("Failed to fetch bindings: {}", e);
            vec![]
        }
    };

    let mut context = tera::Context::new();
    context.insert("SITENAME", "maccms-rust");
    context.insert("bindings", &bindings);

    match TERA.render("admin/bindings.html", &context) {
        Ok(s) => HttpResponse::Ok().content_type("text/html").body(s),
        Err(e) => {
            eprintln!("[ERROR] Failed to render 'admin/bindings.html': {}", e);
            eprintln!("[ERROR] Error kind: {:?}", e.kind);
            eprintln!("[ERROR] Full error chain: {:?}", e);
            HttpResponse::InternalServerError().body("Template error")
        }
    }
}

pub async fn admin_config_page(session: Session, db: web::Data<Database>) -> impl Responder {
    if session.get::<String>("user_id").ok().flatten().is_none() {
        return HttpResponse::Found().append_header(("Location", "/admin/login")).finish();
    }

    let config_collection = db.collection::<crate::models::Config>("configs");
    let configs: Vec<crate::models::Config> = match config_collection.find(None, None).await {
        Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
        Err(e) => {
            eprintln!("Failed to fetch configs: {}", e);
            vec![]
        }
    };

    let mut context = tera::Context::new();
    context.insert("SITENAME", "maccms-rust");
    context.insert("configs", &configs);

    match TERA.render("admin/config.html", &context) {
        Ok(s) => HttpResponse::Ok().content_type("text/html").body(s),
        Err(e) => {
            eprintln!("[ERROR] Failed to render 'admin/config.html': {}", e);
            eprintln!("[ERROR] Error kind: {:?}", e.kind);
            eprintln!("[ERROR] Full error chain: {:?}", e);
            HttpResponse::InternalServerError().body("Template error")
        }
    }
}

pub async fn admin_collect_vod_page(session: Session, db: web::Data<Database>) -> impl Responder {
    // Check if user is logged in
    if session.get::<String>("user_id").unwrap_or(None).is_none() {
        return HttpResponse::Found()
            .append_header(("Location", "/admin/login"))
            .finish();
    }

    let mut context = tera::Context::new();
    
    // 获取采集源列表
    let collections_collection = db.collection::<crate::models::Collection>("collections");
    let collections: Vec<crate::models::Collection> = match collections_collection.find(None, None).await {
        Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
        Err(_) => vec![],
    };
    context.insert("collections", &collections);
    
    // 获取绑定列表
    let bindings_collection = db.collection::<crate::models::Binding>("bindings");
    let bindings: Vec<crate::models::Binding> = match bindings_collection.find(None, None).await {
        Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
        Err(_) => vec![],
    };
    context.insert("bindings", &bindings);
    
    // 获取本地分类列表
    let types_collection = db.collection::<Type>("types");
    let types: Vec<Type> = match types_collection.find(None, None).await {
        Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
        Err(_) => vec![],
    };
    context.insert("types", &types);

    match TERA.render("admin/collect_vod.html", &context) {
        Ok(rendered) => HttpResponse::Ok().content_type("text/html").body(rendered),
        Err(e) => {
            eprintln!("Template rendering error: {}", e);
            eprintln!("Error details: {:?}", e);
            if let Some(source) = e.source() {
                eprintln!("Error source: {}", source);
            }
            HttpResponse::InternalServerError().body(format!("Template rendering failed: {}", e))
        }
    }
}

pub async fn admin_indexes_page(session: Session) -> impl Responder {
    if session.get::<String>("user_id").ok().flatten().is_none() {
        return HttpResponse::Found().append_header(("Location", "/admin/login")).finish();
    }

    let mut context = tera::Context::new();
    context.insert("SITENAME", "maccms-rust");

    match TERA.render("admin/indexes.html", &context) {
        Ok(s) => HttpResponse::Ok().content_type("text/html").body(s),
        Err(e) => {
            eprintln!("[ERROR] Failed to render 'admin/indexes.html': {}", e);
            eprintln!("[ERROR] Error kind: {:?}", e.kind);
            eprintln!("[ERROR] Full error chain: {:?}", e);
            HttpResponse::InternalServerError().body("Template error")
        }
    }
}


