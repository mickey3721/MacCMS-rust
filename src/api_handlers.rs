use actix_web::{web, HttpResponse, Responder};
use mongodb::{Database, bson::doc, options::FindOptions};
use crate::dto::{ApiParams, JsonResponse, VodApiListEntry, Category, VideoFilterParams, CategoryHierarchy};
use crate::models;
use futures::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};

// The main handler for the vod collection API
pub async fn provide_vod(params: web::Query<ApiParams>, db: web::Data<Database>) -> impl Responder {
    // Check for the format parameter, default to JSON
    let format = params.at.as_deref().unwrap_or("json");

    // Build the MongoDB filter based on query parameters
    let mut filter = doc! {};
    if let Some(wd) = &params.wd {
        filter.insert("vod_name", doc! { "$regex": wd, "$options": "i" });
    }
    if let Some(t) = params.t {
        filter.insert("type_id", t);
    }
    // The 'h' parameter logic is temporarily removed due to a dependency issue.
    /*
    if let Some(h) = params.h {
        // Assuming h is in hours, calculate time in the past
        let now = mongodb::bson::DateTime::now();
        let past_time = now.to_chrono() - Duration::hours(h as i64);
        filter.insert("vod_pubdate", doc! { "$gte": past_time });
    }
    */

    // --- Pagination --- 
    let page = params.pg.unwrap_or(1);
    let limit = params.pagesize.unwrap_or(20); // Default page size
    let skip = if page > 0 { (page - 1) * limit } else { 0 };

    let find_options = FindOptions::builder()
        .skip(Some(skip))
        .limit(Some(limit as i64))
        .sort(doc! { "vod_pubdate": -1 })
        .build();

    // --- Database Query --- 
    let vod_collection = db.collection::<models::Vod>("vods");
    let total = match vod_collection.count_documents(filter.clone(), None).await {
        Ok(count) => count,
        Err(_) => return HttpResponse::InternalServerError().body("Failed to count documents"),
    };

    let pagecount = if total > 0 { (total as f64 / limit as f64).ceil() as u64 } else { 0 };

    let cursor = match vod_collection.find(filter, find_options).await {
        Ok(cursor) => cursor,
        Err(_) => return HttpResponse::InternalServerError().body("Failed to fetch videos"),
    };

    let vod_docs: Vec<models::Vod> = match cursor.try_collect().await {
        Ok(docs) => docs,
        Err(_) => return HttpResponse::InternalServerError().body("Failed to collect documents"),
    };

    // --- Data Transformation --- 
    // In a real app, you'd query the Type collection. For now, we'll use a placeholder.
    let list: Vec<VodApiListEntry> = vod_docs.into_iter().map(|vod| {
        VodApiListEntry {
            vod_id: vod.id.unwrap().timestamp().to_string().parse().unwrap_or(0),
            vod_name: vod.vod_name,
            type_id: vod.type_id,
            type_name: Some("N/A".to_string()),
            vod_time: vod.vod_pubdate.to_string(),
            vod_remarks: vod.vod_remarks.unwrap_or_default(),
            vod_play_from: vod.vod_play_urls.into_iter().map(|s| s.source_name).collect::<Vec<_>>().join(","),
            vod_status: Some(vod.vod_status),
            vod_letter: None,
            vod_color: None,
            vod_tag: None,
            vod_class: vod.vod_class,
            vod_pic: vod.vod_pic,
            vod_pic_thumb: None,
            vod_pic_slide: None,
            vod_pic_screenshot: None,
            vod_actor: vod.vod_actor,
            vod_director: vod.vod_director,
            vod_writer: None,
            vod_behind: None,
            vod_blurb: None,
            vod_pubdate: None,
            vod_total: None,
            vod_serial: None,
            vod_tv: None,
            vod_weekday: None,
            vod_area: vod.vod_area,
            vod_lang: vod.vod_lang,
            vod_year: vod.vod_year,
            vod_version: None,
            vod_state: None,
            vod_author: None,
            vod_jumpurl: None,
            vod_tpl: None,
            vod_tpl_play: None,
            vod_tpl_down: None,
            vod_isend: None,
            vod_lock: None,
            vod_level: None,
            vod_copyright: None,
            vod_points: None,
            vod_points_play: None,
            vod_points_down: None,
            vod_hits: None,
            vod_hits_day: None,
            vod_hits_week: None,
            vod_hits_month: None,
            vod_duration: None,
            vod_up: None,
            vod_down: None,
            vod_score: None,
            vod_score_all: None,
            vod_score_num: None,
            vod_time_add: None,
            vod_time_hits: None,
            vod_time_make: None,
            vod_trysee: None,
            vod_douban_id: None,
            vod_douban_score: None,
            vod_reurl: None,
            vod_rel_vod: None,
            vod_rel_art: None,
            vod_pwd: None,
            vod_pwd_url: None,
            vod_pwd_play: None,
            vod_pwd_play_url: None,
            vod_pwd_down: None,
            vod_pwd_down_url: None,
            vod_content: vod.vod_content,
            vod_play_server: None,
            vod_play_note: None,
            vod_play_url: None,
            vod_down_from: None,
            vod_down_server: None,
            vod_down_note: None,
            vod_down_url: None,
        }
    }).collect();

    // --- Category List --- 
    // Placeholder for category list. A real implementation would query the 'types' collection.
    let categories: Vec<Category> = vec![];

    // --- Response Formatting --- 
    if format == "xml" {
        // TODO: Implement XML serialization using quick-xml
        HttpResponse::Ok().content_type("application/xml").body("<rss><list><video><name>XML support coming soon</name></video></list></rss>")
    } else {
        let response = JsonResponse {
            code: 1,
            msg: "success".to_string(),
            page,
            pagecount,
            limit,
            total,
            list,
            categories,
        };
        HttpResponse::Ok().json(response)
    }
}

// API endpoint to get videos by type_id
pub async fn get_videos_by_type(
    path: web::Path<i32>,
    query: web::Query<VideoFilterParams>,
    db: web::Data<Database>,
) -> impl Responder {
    let type_id = path.into_inner();
    let mut filter = doc! { "type_id": type_id };
    
    // Apply additional filters
    if let Some(area) = &query.area {
        filter.insert("vod_area", area);
    }
    if let Some(year) = &query.year {
        filter.insert("vod_year", year);
    }
    if let Some(sub_type) = query.sub_type {
        filter.insert("type_id", sub_type);
    }
    
    // Pagination
    let page = query.pg.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let skip = if page > 0 { (page - 1) * limit } else { 0 };
    
    let find_options = FindOptions::builder()
        .skip(Some(skip))
        .limit(Some(limit as i64))
        .sort(doc! { "vod_pubdate": -1 })
        .build();
    
    let vod_collection = db.collection::<models::Vod>("vods");
    
    let total = match vod_collection.count_documents(filter.clone(), None).await {
        Ok(count) => count,
        Err(_) => return HttpResponse::InternalServerError().body("Failed to count documents"),
    };
    
    let cursor = match vod_collection.find(filter, find_options).await {
        Ok(cursor) => cursor,
        Err(_) => return HttpResponse::InternalServerError().body("Failed to fetch videos"),
    };
    
    let videos: Vec<models::Vod> = match cursor.try_collect().await {
        Ok(docs) => docs,
        Err(_) => return HttpResponse::InternalServerError().body("Failed to collect documents"),
    };
    
    HttpResponse::Ok().json(serde_json::json!({
        "code": 1,
        "msg": "success",
        "page": page,
        "limit": limit,
        "total": total,
        "videos": videos
    }))
}

// API endpoint to get category hierarchy
pub async fn get_category_hierarchy(db: web::Data<Database>) -> impl Responder {
    let type_collection = db.collection::<models::Type>("types");
    
    // Get top-level categories
    let top_categories: Vec<models::Type> = match type_collection.find(doc! { "type_pid": 0 }, None).await {
        Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
        Err(_) => return HttpResponse::InternalServerError().body("Failed to fetch categories"),
    };
    
    let mut hierarchy = Vec::new();
    
    for category in top_categories {
        // Get sub-categories for each top-level category
        let sub_categories: Vec<models::Type> = match type_collection.find(
            doc! { "type_pid": category.type_id }, 
            None
        ).await {
            Ok(cursor) => cursor.try_collect().await.unwrap_or_else(|_| vec![]),
            Err(_) => vec![],
        };
        
        hierarchy.push(CategoryHierarchy {
            category: category.clone(),
            sub_categories,
        });
    }
    
    HttpResponse::Ok().json(serde_json::json!({
        "code": 1,
        "msg": "success",
        "hierarchy": hierarchy
    }))
}

// API endpoint to get video details with play URLs grouped by source
pub async fn get_video_details(
    path: web::Path<String>,
    db: web::Data<Database>,
) -> impl Responder {
    let vod_id = path.into_inner();
    
    let object_id = match mongodb::bson::oid::ObjectId::parse_str(&vod_id) {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().body("Invalid video ID"),
    };
    
    let vod_collection = db.collection::<models::Vod>("vods");
    
    let video = match vod_collection.find_one(doc!{"_id": object_id}, None).await {
        Ok(Some(v)) => v,
        Ok(None) => return HttpResponse::NotFound().body("Video not found"),
        Err(_) => return HttpResponse::InternalServerError().body("Failed to fetch video"),
    };
    
    // Group play URLs by source (already grouped in the model)
    let play_sources = video.vod_play_urls.clone();
    
    HttpResponse::Ok().json(serde_json::json!({
        "code": 1,
        "msg": "success",
        "video": video,
        "play_sources": play_sources
    }))
}

// API endpoint to get unique areas and years for filtering
pub async fn get_filter_options(db: web::Data<Database>) -> impl Responder {
    let vod_collection = db.collection::<models::Vod>("vods");
    
    // Get unique areas
    let areas_pipeline = vec![
        doc! { "$match": { "vod_area": { "$ne": null, "$ne": "" } } },
        doc! { "$group": { "_id": "$vod_area" } },
        doc! { "$sort": { "_id": 1 } }
    ];
    
    let areas: Vec<String> = match vod_collection.aggregate(areas_pipeline, None).await {
        Ok(mut cursor) => {
            let mut result = Vec::new();
            while let Some(doc) = cursor.next().await {
                if let Ok(area_doc) = doc {
                    if let Ok(area) = area_doc.get_str("_id") {
                        result.push(area.to_string());
                    }
                }
            }
            result
        }
        Err(_) => vec![],
    };
    
    // Get unique years
    let years_pipeline = vec![
        doc! { "$match": { "vod_year": { "$ne": null, "$ne": "" } } },
        doc! { "$group": { "_id": "$vod_year" } },
        doc! { "$sort": { "_id": -1 } }
    ];
    
    let years: Vec<String> = match vod_collection.aggregate(years_pipeline, None).await {
        Ok(mut cursor) => {
            let mut result = Vec::new();
            while let Some(doc) = cursor.next().await {
                if let Ok(year_doc) = doc {
                    if let Ok(year) = year_doc.get_str("_id") {
                        result.push(year.to_string());
                    }
                }
            }
            result
        }
        Err(_) => vec![],
    };
    
    HttpResponse::Ok().json(serde_json::json!({
        "code": 1,
        "msg": "success",
        "areas": areas,
        "years": years
    }))
}
