use crate::models::{Type, User, Vod};
use crate::template::TERA;
use actix_web::{web, HttpResponse, Responder};
use futures::stream::TryStreamExt;
use mongodb::{bson::doc, options::FindOptions, Database};
use regex::Regex;

// Helper function to get play URL and episode name
fn get_play_info(
    video: &Vod,
    play_source: usize,
    play_idx: usize,
) -> Result<(String, String), Box<dyn std::error::Error>> {
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

// Helper function to extract line and column information from error messages
fn extract_line_info(error_str: &str) -> Option<String> {
    // 尝试匹配各种可能的行号格式
    if let Some(captures) = regex::Regex::new(r"line (\d+)")
        .ok()?
        .captures(error_str)
    {
        let line = captures.get(1)?.as_str();
        
        // 尝试同时匹配列号
        if let Some(col_captures) = regex::Regex::new(r"column (\d+)")
            .ok()?
            .captures(error_str)
        {
            let column = col_captures.get(1)?.as_str();
            return Some(format!("Line {}, Column {}", line, column));
        }
        
        return Some(format!("Line {}", line));
    }
    
    // 尝试匹配 "at line X" 格式
    if let Some(captures) = regex::Regex::new(r"at line (\d+)")
        .ok()?
        .captures(error_str)
    {
        let line = captures.get(1)?.as_str();
        return Some(format!("Line {}", line));
    }
    
    // 尝试匹配 "(line X)" 格式
    if let Some(captures) = regex::Regex::new(r"\(line (\d+)\)")
        .ok()?
        .captures(error_str)
    {
        let line = captures.get(1)?.as_str();
        return Some(format!("Line {}", line));
    }
    
    None
}
use crate::dto::ListPageParams;
use crate::init_data;
use crate::site_data::SiteDataManager;
use actix_session::Session;
use actix_web_flash_messages::FlashMessage;
use serde::{Deserialize, Serialize};
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
    let categories: Vec<Type> = nav_categories
        .iter()
        .map(|nav| nav.category.clone())
        .collect();
    let categories_with_subs: Vec<(Type, Vec<Type>)> = nav_categories
        .iter()
        .map(|nav| (nav.category.clone(), nav.sub_categories.clone()))
        .collect();

    // 获取所有分类
    let all_categories = site_data_manager.get_all_categories().await;

    // 获取配置数据
    let configs = site_data_manager.get_all_configs().await;

    // 获取网站名称
    let sitename = configs
        .get("site_name")
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



// 通用模板渲染错误处理器
fn handle_template_rendering_error(
    template_name: &str,
    error: &tera::Error,
    context_info: Option<&str>,
    context_variables: Option<&str>,
) {
    eprintln!("🚨 Template Rendering Error");
    eprintln!("");
    eprintln!("=== Template Rendering Error ===");
    eprintln!("");
    eprintln!("Template: {}", template_name);
    eprintln!("");
    eprintln!("Error: {}", error);
    eprintln!("");
    
    // 确定错误类型
    let error_type = match &error.kind {
        tera::ErrorKind::TemplateNotFound(_) => "Template Not Found",
        tera::ErrorKind::Msg(_) => "Template Message Error",
        tera::ErrorKind::CallFunction(_) => "Function Call Error",
        tera::ErrorKind::CallFilter(_) => "Filter Call Error",
        tera::ErrorKind::Json(_) => "JSON Error",
        tera::ErrorKind::Io(_) => "IO Error",
        _ => "Other",
    };
    eprintln!("Error Type: {}", error_type);
    eprintln!("");
    
    // 输出详细的调试信息
    eprintln!("Debug Info: {:?}", error);
    eprintln!("");
    
    // 输出上下文信息
    if let Some(info) = context_info {
        eprintln!("Context Info: {}", info);
        eprintln!("");
    }
    
    // 输出错误链
    let mut level = 1;
    let mut current_error = error.source();
    while let Some(err) = current_error {
        eprintln!("Error Chain Level {}: {}", level, err);
        eprintln!("");
        current_error = err.source();
        level += 1;
    }
    
    // 输出位置信息
    let error_str = format!("{}", error);
    if let Some(line_info) = extract_line_info(&error_str) {
        eprintln!("Error Location: {}", line_info);
        eprintln!("");
    }
    
    // 输出上下文变量信息
    if let Some(variables) = context_variables {
        eprintln!("Context Variables: {}", variables);
        eprintln!("");
    }
    
    // 输出调试建议
    eprintln!("=== Debugging Suggestions ===");
    eprintln!("");
    eprintln!("1. Check if all variables used in the template are properly passed in the context");
    eprintln!("");
    eprintln!("2. Verify template syntax and variable names");
    eprintln!("");
    eprintln!("3. Ensure all required template files exist");
    eprintln!("");
    eprintln!("4. Check for typos in variable names or template");
    eprintln!("");
    
    // 根据错误类型提供特定建议
    match &error.kind {
        tera::ErrorKind::TemplateNotFound(name) => {
            eprintln!("5. Template '{}' not found - check file path and name", name);
            eprintln!("");
        }
        tera::ErrorKind::Msg(msg) if msg.contains("Variable") && msg.contains("not found") => {
            eprintln!("5. Variable not found error - ensure all template variables are provided in context");
            eprintln!("");
        }
        tera::ErrorKind::CallFunction(func_name) => {
            eprintln!("5. Function '{}' call failed - check function implementation and parameters", func_name);
            eprintln!("");
        }
        tera::ErrorKind::CallFilter(filter_name) => {
            eprintln!("5. Filter '{}' call failed - check filter implementation and input data", filter_name);
            eprintln!("");
        }
        _ => {}
    }
}

// 创建包装函数来处理Actix Web路由的参数传递
pub fn with_site_data_wrapper<F, R>(
    handler: F,
) -> impl Fn(web::Data<Database>, web::Data<SiteDataManager>) -> R
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
    match with_site_data(
        db.clone(),
        site_data_manager.clone(),
        |mut context, site_data| async move {
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

                let videos = match vod_collection
                    .find(doc! { "type_id": { "$in": type_ids } }, find_options)
                    .await
                {
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
                .map_err(|e| {
                    handle_template_rendering_error(
                        "index.html",
                        &e,
                        Some("Home page with categorized videos"),
                        Some(&format!("categorized_videos: {} categories", categorized_videos_list.len()))
                    );
                    Box::new(e) as Box<dyn std::error::Error>
                })
        },
    )
    .await
    {
        Ok(response) => response,
        Err(e) => {
            println!("Home page error: {}", e);
            HttpResponse::InternalServerError().body(format!("Error: {}", e))
        }
    }
}

// --- Static Pages Handlers ---

// About page handler
pub async fn about_page(
    db: web::Data<Database>,
    site_data_manager: web::Data<SiteDataManager>,
) -> impl Responder {
    match with_site_data(
        db.clone(),
        site_data_manager.clone(),
        |context, _site_data| async move {
            TERA.render("about.html", &context)
                .map_err(|e| {
                    handle_template_rendering_error(
                        "about.html",
                        &e,
                        Some("About page"),
                        None
                    );
                    Box::new(e) as Box<dyn std::error::Error>
                })
        },
    )
    .await
    {
        Ok(response) => response,
        Err(e) => {
            println!("About page error: {}", e);
            HttpResponse::InternalServerError().body(format!("Error: {}", e))
        }
    }
}

// Contact page handler
pub async fn contact_page(
    db: web::Data<Database>,
    site_data_manager: web::Data<SiteDataManager>,
) -> impl Responder {
    match with_site_data(
        db.clone(),
        site_data_manager.clone(),
        |context, _site_data| async move {
            TERA.render("contact.html", &context)
                .map_err(|e| {
                    handle_template_rendering_error(
                        "contact.html",
                        &e,
                        Some("Contact page"),
                        None
                    );
                    Box::new(e) as Box<dyn std::error::Error>
                })
        },
    )
    .await
    {
        Ok(response) => response,
        Err(e) => {
            println!("Contact page error: {}", e);
            HttpResponse::InternalServerError().body(format!("Error: {}", e))
        }
    }
}

// Privacy page handler
pub async fn privacy_page(
    db: web::Data<Database>,
    site_data_manager: web::Data<SiteDataManager>,
) -> impl Responder {
    match with_site_data(
        db.clone(),
        site_data_manager.clone(),
        |context, _site_data| async move {
            TERA.render("privacy.html", &context)
                .map_err(|e| {
                    handle_template_rendering_error(
                        "privacy.html",
                        &e,
                        Some("Privacy page"),
                        None
                    );
                    Box::new(e) as Box<dyn std::error::Error>
                })
        },
    )
    .await
    {
        Ok(response) => response,
        Err(e) => {
            println!("Privacy page error: {}", e);
            HttpResponse::InternalServerError().body(format!("Error: {}", e))
        }
    }
}

// Terms page handler
pub async fn terms_page(
    db: web::Data<Database>,
    site_data_manager: web::Data<SiteDataManager>,
) -> impl Responder {
    match with_site_data(
        db.clone(),
        site_data_manager.clone(),
        |context, _site_data| async move {
            TERA.render("terms.html", &context)
                .map_err(|e| {
                    handle_template_rendering_error(
                        "terms.html",
                        &e,
                        Some("Terms of service page"),
                        None
                    );
                    Box::new(e) as Box<dyn std::error::Error>
                })
        },
    )
    .await
    {
        Ok(response) => response,
        Err(e) => {
            println!("Terms page error: {}", e);
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

    match with_site_data(
        db.clone(),
        site_data_manager.clone(),
        |mut context, site_data| async move {
            let vod_collection = db.collection::<Vod>("vods");

            // 1. Fetch video details
            let video = match vod_collection.find_one(doc! {"_id": object_id}, None).await {
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

            let related_videos: Vec<Vod> = match vod_collection
                .find(
                    doc! { "type_id": video.type_id, "_id": { "$ne": object_id } },
                    find_options,
                )
                .await
            {
                Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
                Err(_) => vec![],
            };

            // Convert related videos dates to timestamps
            let related_timestamps: Vec<i64> = related_videos
                .iter()
                .map(|v| v.vod_pubdate.timestamp_millis() / 1000)
                .collect();
            context.insert("related_videos", &related_videos);
            context.insert("related_pubdate_timestamps", &related_timestamps);

            TERA.render("detail.html", &context)
                .map_err(|e| {
                    let context_variables = format!(
                        "video: {}, category: {}, related_videos: {} items",
                        video.vod_name,
                        context.get("category").map_or("None".to_string(), |_| "Available".to_string()),
                        related_videos.len()
                    );
                    
                    handle_template_rendering_error(
                        "detail.html",
                        &e,
                        Some("Video detail page with related videos"),
                        Some(&context_variables)
                    );
                    Box::new(e) as Box<dyn std::error::Error>
                })
        },
    )
    .await
    {
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

    match with_site_data(
        db.clone(),
        site_data_manager.clone(),
        |mut context, site_data| async move {
            let vod_collection = db.collection::<Vod>("vods");

            // 1. Fetch video details and increment hit count
            let video = match vod_collection.find_one(doc! {"_id": object_id}, None).await {
                Ok(Some(v)) => v,
                _ => return Err("Video not found".into()),
            };

            // Increment hit count
            let current_hits = video.vod_hits.unwrap_or(0);
            let current_hits_day = video.vod_hits_day.unwrap_or(0);
            let current_hits_week = video.vod_hits_week.unwrap_or(0);
            let current_hits_month = video.vod_hits_month.unwrap_or(0);

            let update_result = vod_collection
                .update_one(
                    doc! {"_id": object_id},
                    doc! {"$set": {
                        "vod_hits": current_hits + 1,
                        "vod_hits_day": current_hits_day + 1,
                        "vod_hits_week": current_hits_week + 1,
                        "vod_hits_month": current_hits_month + 1,
                    }},
                    None,
                )
                .await;

            if let Err(e) = update_result {
                println!("Warning: Failed to update hit count: {}", e);
            }

            // Convert MongoDB DateTime to timestamp for template
            let pubdate_timestamp = video.vod_pubdate.timestamp_millis() / 1000;
            context.insert("vod_pubdate_timestamp", &pubdate_timestamp);
            context.insert("video", &video);

            // 2. Get play URL and episode name
            let (play_url, current_episode_name) =
                match get_play_info(&video, play_source, play_idx) {
                    Ok(info) => info,
                    Err(e) => return Err(e),
                };

            context.insert("play_url", &play_url);
            context.insert("play_index", &play_idx);
            context.insert("play_source", &play_source);
            context.insert("current_episode_name", &current_episode_name);

            // 3. Get recommended movies (same category, excluding current video)
            let find_options = FindOptions::builder()
                .sort(doc! { "vod_pubdate": -1 })
                .limit(6)
                .build();

            let recommended_movies: Vec<Vod> = match vod_collection
                .find(
                    doc! { "type_id": video.type_id, "_id": { "$ne": object_id } },
                    find_options,
                )
                .await
            {
                Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
                Err(_) => vec![],
            };

            // Convert recommended videos dates to timestamps
            let recommended_timestamps: Vec<i64> = recommended_movies
                .iter()
                .map(|v| v.vod_pubdate.timestamp_millis() / 1000)
                .collect();

            context.insert("recommended_movies", &recommended_movies);
            context.insert("recommended_timestamps", &recommended_timestamps);

            TERA.render("player.html", &context).map_err(|e| {
                let context_variables = format!(
                    "video: {}, video.id: {:?}, play_url: {}, play_index: {}, play_source: {}, current_episode_name: {}, vod_pubdate_timestamp: {:?}, video_sources: {} sources",
                    video.vod_name,
                    video.id,
                    play_url,
                    play_idx,
                    play_source,
                    current_episode_name,
                    pubdate_timestamp,
                    video.vod_play_urls.len()
                );
                
                handle_template_rendering_error(
                    "player.html",
                    &e,
                    Some("Video player page with play sources and recommendations"),
                    Some(&context_variables)
                );
                
                Box::new(e) as Box<dyn std::error::Error>
            })
        },
    )
    .await
    {
        Ok(response) => response,
        Err(e) => {
            eprintln!("Video player handler error: {}", e);
            HttpResponse::InternalServerError()
                .body(format!("Failed to render 'player.html': {}", e))
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

    match with_site_data(
        db.clone(),
        site_data_manager.clone(),
        |mut context, site_data| async move {
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
                Some("hits") => doc! { "vod_hits": -1 },      // Most played
                Some("score") => doc! { "vod_score": -1 },    // Highest rated
                Some("year_desc") => doc! { "vod_year": -1 }, // Newest year
                Some("year_asc") => doc! { "vod_year": 1 },   // Oldest year
                Some("name_asc") => doc! { "vod_name": 1 },   // Name A-Z
                Some("name_desc") => doc! { "vod_name": -1 }, // Name Z-A
                _ => doc! { "vod_pubdate": -1 },              // Default: latest published
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
                let end_page = if page + 3 < total_pages {
                    page + 3
                } else {
                    total_pages
                };

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
        },
    )
    .await
    {
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
    match with_site_data(
        db.clone(),
        site_data_manager.clone(),
        |mut context, _site_data| async move {
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
                .map_err(|e| {
                    let context_variables = format!(
                        "search_results count: {}, search_keyword: {:?}",
                        search_results.len(),
                        query.wd
                    );
                    
                    handle_template_rendering_error(
                        "search.html",
                        &e,
                        Some("Search results page"),
                        Some(&context_variables)
                    );
                    Box::new(e) as Box<dyn std::error::Error>
                })
        },
    )
    .await
    {
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
            handle_template_rendering_error(
                "admin/login.html",
                &e,
                Some("Admin login page"),
                None
            );
            HttpResponse::InternalServerError().body("Template error")
        }
    }
}

pub async fn login_post(
    db: web::Data<Database>,
    form: web::Form<LoginForm>,
    session: Session,
) -> impl Responder {
    println!(
        "[DEBUG] Login attempt - Username: '{}', Password length: {}",
        form.username,
        form.password.len()
    );

    let user_collection = db.collection::<User>("users");

    let user = match user_collection
        .find_one(doc! {"user_name": &form.username}, None)
        .await
    {
        Ok(Some(u)) => {
            println!("[DEBUG] User found in database: {}", u.user_name);
            u
        }
        Ok(None) => {
            println!(
                "[DEBUG] User not found in database for username: {}",
                form.username
            );
            FlashMessage::error("Invalid username or password.").send();
            return HttpResponse::Found()
                .append_header(("Location", "/admin/login"))
                .finish();
        }
        Err(e) => {
            println!("[DEBUG] Database error when finding user: {}", e);
            FlashMessage::error("Invalid username or password.").send();
            return HttpResponse::Found()
                .append_header(("Location", "/admin/login"))
                .finish();
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
                HttpResponse::Found()
                    .append_header(("Location", "/admin"))
                    .finish()
            }
            Err(e) => {
                println!("[DEBUG] Failed to set session: {}", e);
                FlashMessage::error("Login failed due to session error.").send();
                HttpResponse::Found()
                    .append_header(("Location", "/admin/login"))
                    .finish()
            }
        }
    } else {
        println!("[DEBUG] Password verification failed, redirecting back to login");
        FlashMessage::error("Invalid username or password.").send();
        HttpResponse::Found()
            .append_header(("Location", "/admin/login"))
            .finish()
    }
}

pub async fn admin_dashboard(session: Session, db: web::Data<Database>) -> impl Responder {
    if session.get::<String>("user_id").ok().flatten().is_none() {
        return HttpResponse::Found()
            .append_header(("Location", "/admin/login"))
            .finish();
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
    if let Ok(count) = db
        .collection::<mongodb::bson::Document>("vods")
        .count_documents(None, None)
        .await
    {
        total_videos = count as i32;
    }

    // 获取分类数量
    if let Ok(count) = db
        .collection::<mongodb::bson::Document>("types")
        .count_documents(None, None)
        .await
    {
        total_categories = count as i32;
    }

    // 获取采集源数量
    if let Ok(count) = db
        .collection::<mongodb::bson::Document>("collections")
        .count_documents(None, None)
        .await
    {
        total_collections = count as i32;
    }

    // 获取配置数量
    if let Ok(count) = db
        .collection::<mongodb::bson::Document>("configs")
        .count_documents(None, None)
        .await
    {
        total_configs = count as i32;
    }

    // 获取绑定数量
    if let Ok(count) = db
        .collection::<mongodb::bson::Document>("bindings")
        .count_documents(None, None)
        .await
    {
        total_bindings = count as i32;
    }

    // 获取用户数量
    if let Ok(count) = db
        .collection::<mongodb::bson::Document>("users")
        .count_documents(None, None)
        .await
    {
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
            let context_variables = format!(
                "total_videos: {}, total_categories: {}, total_collections: {}, total_configs: {}, total_bindings: {}, total_users: {}",
                total_videos, total_categories, total_collections, total_configs, total_bindings, total_users
            );
            
            handle_template_rendering_error(
                "admin/index.html",
                &e,
                Some("Admin dashboard with statistics"),
                Some(&context_variables)
            );
            HttpResponse::InternalServerError().body("Template error")
        }
    }
}

pub async fn logout(session: Session) -> impl Responder {
    session.clear();
    HttpResponse::Found()
        .append_header(("Location", "/admin/login"))
        .finish()
}

pub async fn admin_types_page(session: Session, db: web::Data<Database>) -> impl Responder {
    if session.get::<String>("user_id").ok().flatten().is_none() {
        return HttpResponse::Found()
            .append_header(("Location", "/admin/login"))
            .finish();
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
        return HttpResponse::Found()
            .append_header(("Location", "/admin/login"))
            .finish();
    }

    match init_data::init_all_data(&db).await {
        Ok(_) => {
            FlashMessage::info("数据初始化成功！").send();
            HttpResponse::Found()
                .append_header(("Location", "/admin"))
                .finish()
        }
        Err(e) => {
            eprintln!("Data initialization failed: {}", e);
            FlashMessage::error(&format!("数据初始化失败: {}", e)).send();
            HttpResponse::Found()
                .append_header(("Location", "/admin"))
                .finish()
        }
    }
}

pub async fn admin_vods_page(session: Session, db: web::Data<Database>) -> impl Responder {
    if session.get::<String>("user_id").ok().flatten().is_none() {
        return HttpResponse::Found()
            .append_header(("Location", "/admin/login"))
            .finish();
    }

    let vod_collection = db.collection::<Vod>("vods");
    let find_options = FindOptions::builder()
        .sort(doc! {"vod_pubdate": -1})
        .limit(50)
        .build();
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
            handle_template_rendering_error(
                "admin/vods.html",
                &e,
                Some("Admin VOD management page"),
                None
            );
            HttpResponse::InternalServerError().body("Template error")
        }
    }
}

pub async fn admin_collect_page(session: Session, db: web::Data<Database>) -> impl Responder {
    if session.get::<String>("user_id").ok().flatten().is_none() {
        return HttpResponse::Found()
            .append_header(("Location", "/admin/login"))
            .finish();
    }
    let collection_collection = db.collection::<crate::models::Collection>("collections");
    let collections: Vec<crate::models::Collection> =
        match collection_collection.find(None, None).await {
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
            let context_variables = format!("collections count: {}", collections.len());
            
            handle_template_rendering_error(
                "admin/collect.html",
                &e,
                Some("Admin collection management page"),
                Some(&context_variables)
            );
            HttpResponse::InternalServerError().body("Template error")
        }
    }
}

pub async fn admin_bindings_page(session: Session, db: web::Data<Database>) -> impl Responder {
    if session.get::<String>("user_id").ok().flatten().is_none() {
        return HttpResponse::Found()
            .append_header(("Location", "/admin/login"))
            .finish();
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
            let context_variables = format!("bindings count: {}", bindings.len());
            
            handle_template_rendering_error(
                "admin/bindings.html",
                &e,
                Some("Admin bindings management page"),
                Some(&context_variables)
            );
            HttpResponse::InternalServerError().body("Template error")
        }
    }
}

pub async fn admin_config_page(session: Session, db: web::Data<Database>) -> impl Responder {
    if session.get::<String>("user_id").ok().flatten().is_none() {
        return HttpResponse::Found()
            .append_header(("Location", "/admin/login"))
            .finish();
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
            let context_variables = format!("configs count: {}", configs.len());
            
            handle_template_rendering_error(
                "admin/config.html",
                &e,
                Some("Admin configuration management page"),
                Some(&context_variables)
            );
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
    let collections: Vec<crate::models::Collection> =
        match collections_collection.find(None, None).await {
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
            let context_variables = format!(
                "collections count: {}, bindings count: {}, types count: {}",
                collections.len(), bindings.len(), types.len()
            );
            
            handle_template_rendering_error(
                "admin/collect_vod.html",
                &e,
                Some("Admin VOD collection page"),
                Some(&context_variables)
            );
            HttpResponse::InternalServerError().body("Template error")
        }
    }
}

pub async fn admin_indexes_page(session: Session) -> impl Responder {
    if session.get::<String>("user_id").ok().flatten().is_none() {
        return HttpResponse::Found()
            .append_header(("Location", "/admin/login"))
            .finish();
    }

    let mut context = tera::Context::new();
    context.insert("SITENAME", "maccms-rust");

    match TERA.render("admin/indexes.html", &context) {
        Ok(s) => HttpResponse::Ok().content_type("text/html").body(s),
        Err(e) => {
            handle_template_rendering_error(
                "admin/indexes.html",
                &e,
                Some("Admin indexes management page"),
                None
            );
            HttpResponse::InternalServerError().body("Template error")
        }
    }
}

// 刷新缓存处理器
pub async fn refresh_cache_handler(
    session: Session,
    site_data_manager: web::Data<SiteDataManager>,
) -> impl Responder {
    // 检查用户是否登录
    if session.get::<String>("user_id").ok().flatten().is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({
            "success": false,
            "message": "未登录或会话已过期"
        }));
    }

    match site_data_manager.refresh().await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "message": "缓存刷新成功",
            "timestamp": chrono::Utc::now().timestamp()
        })),
        Err(e) => {
            eprintln!("Cache refresh failed: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "message": format!("缓存刷新失败: {}", e)
            }))
        }
    }
}
