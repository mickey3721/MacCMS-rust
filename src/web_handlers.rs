use actix_web::{web, HttpResponse, Responder};
use mongodb::{Database, bson::doc, options::FindOptions};
use crate::template::TERA;
use crate::models::{Type, Vod, User};
use futures::stream::TryStreamExt;

// Helper function to get play URL and episode name
fn get_play_info(video: &Vod, play_source: usize, play_idx: usize) -> Result<(String, String), Box<dyn std::error::Error>> {
    if let Some(source) = video.vod_play_urls.get(play_source) {
        if let Some(url_info) = source.urls.get(play_idx) {
            Ok((url_info.url.clone(), url_info.name.clone()))
        } else {
            Err("Play URL not found".into())
        }
    } else {
        Err("No play sources available".into())
    }
}
use serde::{Serialize, Deserialize};
use crate::dto::ListPageParams;
use actix_session::Session;
use actix_web_flash_messages::FlashMessage;
use crate::init_data;
use crate::site_data::SiteDataManager;
use std::error::Error;

#[derive(Serialize)]
struct CategorizedVideos {
    category: Type,
    videos: Vec<Vod>,
}

// 辅助函数：获取站点数据并添加到模板上下文
async fn with_site_data<F, R>(
    db: web::Data<Database>,
    site_data_manager: web::Data<SiteDataManager>,
    template_handler: F,
) -> Result<HttpResponse, Box<dyn std::error::Error>>
where
    F: FnOnce(tera::Context, SiteDataManager) -> R,
    R: std::future::Future<Output = Result<String, Box<dyn std::error::Error>>>,
{
    let mut context = tera::Context::new();
    
    // 获取导航分类数据
    let nav_categories = site_data_manager.get_navigation_categories().await;
    let categories: Vec<Type> = nav_categories.iter().map(|nav| nav.category.clone()).collect();
    let categories_with_subs: Vec<(Type, Vec<Type>)> = nav_categories.iter()
        .map(|nav| (nav.category.clone(), nav.sub_categories.clone()))
        .collect();
    
    // 获取所有分类
    let all_categories = site_data_manager.get_all_categories().await;
    
    // 获取配置数据
    let configs = site_data_manager.get_all_configs().await;
    
    // 获取网站名称
    let sitename = configs.get("site_name")
        .cloned()
        .unwrap_or_else(|| "maccms-rust".to_string());
    
    // 添加全局数据到上下文
    context.insert("types", &all_categories);
    context.insert("categories", &categories);
    context.insert("categories_with_subs", &categories_with_subs);
    context.insert("configs", &configs);
    context.insert("SITENAME", &sitename);
    
    // 为方便模板使用，添加一些常用的配置项
    if let Some(site_url) = configs.get("site_url") {
        context.insert("SITEURL", site_url);
    }
    if let Some(site_keywords) = configs.get("site_keywords") {
        context.insert("SITEKEYWORDS", site_keywords);
    }
    if let Some(site_description) = configs.get("site_description") {
        context.insert("SITEDESCRIPTION", site_description);
    }
    
    let rendered = template_handler(context, site_data_manager.as_ref().clone()).await?;
    
    Ok(HttpResponse::Ok().content_type("text/html").body(rendered))
}

// 辅助函数：处理模板渲染错误
fn handle_template_error(e: tera::Error) -> HttpResponse {
    println!("Template rendering error: {}", e);
    println!("Error details: {:?}", e);
    HttpResponse::InternalServerError().body(format!("Template error: {}", e))
}

// 创建包装函数来处理Actix Web路由的参数传递
pub fn with_site_data_wrapper<F, R>(handler: F) -> impl Fn(web::Data<Database>, web::Data<SiteDataManager>) -> R
where
    F: Fn(web::Data<Database>, web::Data<SiteDataManager>) -> R,
    R: std::future::Future,
{
    handler
}

// 具体的包装函数
pub async fn home_page_wrapper(
    db: web::Data<Database>,
    site_data_manager: web::Data<SiteDataManager>,
) -> impl Responder {
    home_page(db, site_data_manager).await
}

pub async fn video_detail_handler_wrapper(
    path: web::Path<String>,
    db: web::Data<Database>,
    site_data_manager: web::Data<SiteDataManager>,
) -> impl Responder {
    video_detail_handler(path, db, site_data_manager).await
}

pub async fn video_player_handler_wrapper(
    path: web::Path<(String, String)>,
    db: web::Data<Database>,
    site_data_manager: web::Data<SiteDataManager>,
) -> impl Responder {
    video_player_handler(path, db, site_data_manager).await
}

pub async fn list_page_handler_wrapper(
    path: web::Path<i32>,
    query: web::Query<ListPageParams>,
    db: web::Data<Database>,
    site_data_manager: web::Data<SiteDataManager>,
) -> impl Responder {
    list_page_handler(path, query, db, site_data_manager).await
}

pub async fn search_page_handler_wrapper(
    query: web::Query<crate::dto::ApiParams>,
    db: web::Data<Database>,
    site_data_manager: web::Data<SiteDataManager>,
) -> impl Responder {
    search_page_handler(query, db, site_data_manager).await
}

// --- Frontend Web Handlers ---

pub async fn home_page(
    db: web::Data<Database>,
    site_data_manager: web::Data<SiteDataManager>,
) -> impl Responder {
    match with_site_data(db.clone(), site_data_manager.clone(), |mut context, site_data| async move {
        let vod_collection = db.collection::<Vod>("vods");
        let mut categorized_videos_list = Vec::new();

        // 获取导航分类数据
        let nav_categories = site_data.get_navigation_categories().await;

        // Fetch videos for each top-level category (include sub-categories)
        for nav_category in nav_categories {
            let find_options = FindOptions::builder()
                .sort(doc! { "vod_pubdate": -1 })
                .limit(12)
                .build();
            
            // Build filter to include both top-level category and its sub-categories
            let mut type_ids = vec![nav_category.category.type_id];
            for sub_cat in &nav_category.sub_categories {
                type_ids.push(sub_cat.type_id);
            }
            
            let videos = match vod_collection.find(doc! { "type_id": { "$in": type_ids } }, find_options).await {
                Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
                Err(_) => vec![],
            };

            categorized_videos_list.push(CategorizedVideos {
                category: nav_category.category,
                videos,
            });
        }

        context.insert("categorized_videos", &categorized_videos_list);
        
        TERA.render("index.html", &context)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }).await {
        Ok(response) => response,
        Err(e) => {
            println!("Home page error: {}", e);
            HttpResponse::InternalServerError().body(format!("Error: {}", e))
        }
    }
}

// Video detail page handler
pub async fn video_detail_handler(
    path: web::Path<String>,
    db: web::Data<Database>,
    site_data_manager: web::Data<SiteDataManager>,
) -> impl Responder {
    let vod_id = path.into_inner();
    
    // Parse ObjectId from string
    let object_id = match mongodb::bson::oid::ObjectId::parse_str(&vod_id) {
        Ok(id) => id,
        Err(_) => return HttpResponse::NotFound().body("Invalid video ID"),
    };

    match with_site_data(db.clone(), site_data_manager.clone(), |mut context, site_data| async move {
        let vod_collection = db.collection::<Vod>("vods");

        // 1. Fetch video details
        let video = match vod_collection.find_one(doc!{"_id": object_id}, None).await {
            Ok(Some(v)) => v,
            _ => return Err("Video not found".into()),
        };
        
        // Convert MongoDB DateTime to timestamp for template
        let pubdate_timestamp = video.vod_pubdate.timestamp_millis() / 1000;
        context.insert("vod_pubdate_timestamp", &pubdate_timestamp);
        context.insert("video", &video);

        // 2. Fetch category info
        if let Some(category) = site_data.get_category_by_id(video.type_id).await {
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

        TERA.render("detail.html", &context)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }).await {
        Ok(response) => response,
        Err(e) => {
            println!("Video detail error: {}", e);
            HttpResponse::InternalServerError().body(format!("Error: {}", e))
        }
    }
}

// Video player page handler
pub async fn video_player_handler(
    path: web::Path<(String, String)>,
    db: web::Data<Database>,
    site_data_manager: web::Data<SiteDataManager>,
) -> impl Responder {
    let (vod_id, play_index) = path.into_inner();
    
    // Parse ObjectId from string
    let object_id = match mongodb::bson::oid::ObjectId::parse_str(&vod_id) {
        Ok(id) => id,
        Err(_) => return HttpResponse::NotFound().body("Invalid video ID"),
    };

    // Parse play index (format: "source-index" or "index")
    let (play_source, play_idx) = if play_index.contains('-') {
        // Format: "source-index", extract both parts
        let parts: Vec<&str> = play_index.split('-').collect();
        if parts.len() != 2 {
            return HttpResponse::NotFound().body("Invalid play index format");
        }
        let source_idx = match parts[0].parse() {
            Ok(idx) => idx,
            Err(_) => return HttpResponse::NotFound().body("Invalid source index"),
        };
        let episode_idx = match parts[1].parse() {
            Ok(idx) => idx,
            Err(_) => return HttpResponse::NotFound().body("Invalid episode index"),
        };
        (source_idx, episode_idx)
    } else {
        // Format: "index" (backward compatibility, default to source 0)
        let episode_idx = match play_index.parse() {
            Ok(idx) => idx,
            Err(_) => return HttpResponse::NotFound().body("Invalid play index"),
        };
        (0, episode_idx)
    };

    match with_site_data(db.clone(), site_data_manager.clone(), |mut context, site_data| async move {
        let vod_collection = db.collection::<Vod>("vods");

        // 1. Fetch video details and increment hit count
        let video = match vod_collection.find_one(doc!{"_id": object_id}, None).await {
            Ok(Some(v)) => v,
            _ => return Err("Video not found".into()),
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

        // 2. Get play URL and episode name
        let (play_url, current_episode_name) = match get_play_info(&video, play_source, play_idx) {
            Ok(info) => info,
            Err(e) => return Err(e),
        };

        context.insert("play_url", &play_url);
        context.insert("play_index", &play_idx);
        context.insert("play_source", &play_source);
        context.insert("current_episode_name", &current_episode_name);

        TERA.render("player.html", &context)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }).await {
        Ok(response) => response,
        Err(e) => {
            println!("Video player error: {}", e);
            HttpResponse::InternalServerError().body(format!("Error: {}", e))
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
    site_data_manager: web::Data<SiteDataManager>,
) -> impl Responder {
    let type_id = path.into_inner();

    match with_site_data(db.clone(), site_data_manager.clone(), |mut context, site_data| async move {
        // Get main category info
        let main_category = match site_data.get_category_by_id(type_id).await {
            Some(cat) => cat,
            None => return Err("Category not found".into()),
        };
        context.insert("category", &main_category);

        // Determine the actual category for filtering options (use parent if this is a sub-category)
        let filter_category = if main_category.type_pid == 0 {
            // This is already a top-level category
            main_category.clone()
        } else {
            // This is a sub-category, get its parent for filtering options
            match site_data.get_category_by_id(main_category.type_pid).await {
                Some(parent_cat) => parent_cat,
                None => return Err("Parent category not found".into()),
            }
        };

        // Parse subarea and subyear for filtering options from the filter_category
        let subarea_options: Vec<String> = if let Some(subarea) = &filter_category.subarea {
            subarea.split(',').map(|s| s.trim().to_string()).collect()
        } else {
            vec![]
        };
        let subyear_options: Vec<String> = if let Some(subyear) = &filter_category.subyear {
            subyear.split(',').map(|s| s.trim().to_string()).collect()
        } else {
            vec![]
        };
        context.insert("subarea_options", &subarea_options);
        context.insert("subyear_options", &subyear_options);

        // Get sub-categories for filter (from the filter_category if it's a top-level category)
        let all_categories = site_data.get_all_categories().await;
        let sub_categories: Vec<Type> = all_categories
            .iter()
            .filter(|cat| cat.type_pid == filter_category.type_id)
            .cloned()
            .collect();
        context.insert("sub_categories", &sub_categories);

        // Initialize filter variables for template
        context.insert("current_sub_type", &None::<i32>);
        context.insert("current_area", &None::<String>);
        context.insert("current_year", &None::<String>);
        context.insert("current_sort", &query.sort);

        let vod_collection = db.collection::<Vod>("vods");

        // Build filter for videos
        let mut filter = doc! {};
        
        // Handle sub_type filtering - if sub_type is provided, use it instead of main type_id
        let mut display_category = main_category.clone();
        if let Some(sub_type) = query.sub_type {
            context.insert("current_sub_type", &sub_type);
            filter.insert("type_id", sub_type);
            
            // Fetch subcategory info for SEO and display
            if let Some(sub_cat) = site_data.get_category_by_id(sub_type).await {
                display_category = sub_cat;
                context.insert("subcategory", &display_category);
            }
        } else {
            // If no sub_type is selected, include main category and all its sub-categories
            let mut type_ids = vec![type_id];
            for sub_cat in &sub_categories {
                type_ids.push(sub_cat.type_id);
            }
            filter.insert("type_id", doc! { "$in": type_ids });
        }
        
        // Always insert the display category (either main category or subcategory)
        context.insert("display_category", &display_category);

        if let Some(area) = &query.area {
            if !area.is_empty() {
                filter.insert("vod_area", area);
                context.insert("current_area", area);
            }
        }
        if let Some(year) = &query.year {
            if !year.is_empty() {
                filter.insert("vod_year", year);
                context.insert("current_year", year);
            }
        }

        // Pagination setup
        let page = query.pg.unwrap_or(1);
        let limit = 20; // Items per page
        let skip = if page > 0 { (page - 1) * limit } else { 0 };

        // Count total documents for pagination
        let total_items = match vod_collection.count_documents(filter.clone(), None).await {
            Ok(count) => count,
            Err(_) => 0,
        };
        
        let total_pages = if total_items > 0 {
            (total_items as f64 / limit as f64).ceil() as u64
        } else {
            0
        };

        // Build sort options based on query parameter
        let sort_doc = match query.sort.as_deref() {
            Some("hits") => doc! { "vod_hits": -1 }, // Most played
            Some("score") => doc! { "vod_score": -1 }, // Highest rated
            Some("year_desc") => doc! { "vod_year": -1 }, // Newest year
            Some("year_asc") => doc! { "vod_year": 1 }, // Oldest year
            Some("name_asc") => doc! { "vod_name": 1 }, // Name A-Z
            Some("name_desc") => doc! { "vod_name": -1 }, // Name Z-A
            _ => doc! { "vod_pubdate": -1 }, // Default: latest published
        };

        // Fetch videos based on filter with pagination
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
        
        // Add total items count to context
        context.insert("total_items", &total_items);

        // Add pagination info to context
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

        TERA.render("list.html", &context)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }).await {
        Ok(response) => response,
        Err(e) => {
            println!("List page error: {}", e);
            HttpResponse::InternalServerError().body(format!("Error: {}", e))
        }
    }
}

// Search page handler
pub async fn search_page_handler(
    query: web::Query<crate::dto::ApiParams>,
    db: web::Data<Database>,
    site_data_manager: web::Data<SiteDataManager>,
) -> impl Responder {
    match with_site_data(db.clone(), site_data_manager.clone(), |mut context, _site_data| async move {
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

        TERA.render("search.html", &context)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }).await {
        Ok(response) => response,
        Err(e) => {
            println!("Search page error: {}", e);
            HttpResponse::InternalServerError().body(format!("Error: {}", e))
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

// 刷新缓存处理器
pub async fn refresh_cache_handler(session: Session, site_data_manager: web::Data<SiteDataManager>) -> impl Responder {
    // 检查用户是否登录
    if session.get::<String>("user_id").ok().flatten().is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({
            "success": false,
            "message": "未登录或会话已过期"
        }));
    }

    match site_data_manager.refresh().await {
        Ok(_) => {
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "message": "缓存刷新成功",
                "timestamp": chrono::Utc::now().timestamp()
            }))
        }
        Err(e) => {
            eprintln!("Cache refresh failed: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "message": format!("缓存刷新失败: {}", e)
            }))
        }
    }
}


