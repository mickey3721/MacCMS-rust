use actix_session::Session;
use actix_web::{web, HttpResponse, Responder};
use futures::stream::TryStreamExt;
use mongodb::{
    bson::doc,
    options::{FindOneOptions, FindOptions},
    Database,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::index_manager::{IndexManager, CollectionIndexInfo, SingleIndexInfo};
use crate::models::{Binding, Collection, Config, Type, Vod};

// Helper function to check if user is authenticated
fn check_auth(session: &Session) -> Result<(), HttpResponse> {
    match session.get::<String>("user_id") {
        Ok(Some(_)) => Ok(()),
        _ => Err(HttpResponse::Unauthorized().json(json!({
            "error": "Unauthorized",
            "message": "Please login to access this resource"
        }))),
    }
}

// --- DTOs for Admin API ---
#[derive(Debug, Serialize, Deserialize)]
pub struct TypeRequest {
    pub type_name: String,
    pub type_pid: i32,
    pub type_en: Option<String>,
    pub type_sort: Option<i32>,
    pub type_status: Option<i32>,
    pub type_mid: Option<i32>,
    pub type_key: Option<String>,
    pub type_des: Option<String>,
    pub type_title: Option<String>,
    pub subarea: Option<String>,
    pub subyear: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BindingRequest {
    pub source_flag: String, // e.g., "my_api_source"
    pub external_id: String, // e.g., "123"
    pub local_type_id: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigRequest {
    pub config_key: String,
    pub config_value: String,
    pub config_desc: Option<String>,
    pub config_type: String,
    pub config_group: Option<String>,
    pub config_sort: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionRequest {
    pub collect_name: String,
    pub collect_url: String,
    pub collect_type: i32,
    pub collect_mid: i32,
    pub collect_appid: String,
    pub collect_appkey: String,
    pub collect_param: String,
    pub collect_filter: String,
    #[serde(default)]
    pub collect_filter_from: String,
    pub collect_opt: i32,
    pub collect_sync_pic_opt: i32,
    pub collect_remove_ad: i32,
    pub collect_convert_webp: i32,
    pub collect_download_retry: i32,
    pub collect_status: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VodRequest {
    pub vod_name: String,
    pub type_id: i32,
    pub vod_status: i32,
    pub vod_class: Option<String>,
    pub vod_pic: Option<String>,
    pub vod_actor: Option<String>,
    pub vod_director: Option<String>,
    pub vod_remarks: Option<String>,
    pub vod_area: Option<String>,
    pub vod_lang: Option<String>,
    pub vod_year: Option<String>,
    pub vod_content: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchDeleteRequest {
    pub ids: Vec<String>,
}

// --- Category Management API ---

// GET /api/admin/types
pub async fn get_types(db: web::Data<Database>, session: Session) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }
    let collection = db.collection::<Type>("types");
    let find_options = FindOptions::builder().sort(doc! {"type_sort": 1}).build();

    match collection.find(None, find_options).await {
        Ok(cursor) => {
            let types: Vec<Type> = cursor.try_collect().await.unwrap_or_else(|_| vec![]);
            HttpResponse::Ok().json(types)
        }
        Err(e) => {
            eprintln!("Failed to fetch types: {}", e);
            HttpResponse::InternalServerError().body("Failed to fetch types")
        }
    }
}

// --- Collection Management API ---

// GET /api/admin/collections
pub async fn get_collections(db: web::Data<Database>, session: Session) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }
    let collection = db.collection::<Collection>("collections");
    let find_options = FindOptions::builder().sort(doc! {"created_at": -1}).build();

    match collection.find(None, find_options).await {
        Ok(cursor) => {
            let collections: Vec<Collection> =
                cursor.try_collect().await.unwrap_or_else(|_| vec![]);
            HttpResponse::Ok().json(collections)
        }
        Err(e) => {
            eprintln!("Failed to fetch collections: {}", e);
            HttpResponse::InternalServerError().body("Failed to fetch collections")
        }
    }
}

// POST /api/admin/collections
pub async fn create_collection(
    db: web::Data<Database>,
    collection_req: web::Json<CollectionRequest>,
    session: Session,
) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }
    let collection = db.collection::<Collection>("collections");

    let new_collection = Collection {
        id: None,
        collect_name: collection_req.collect_name.clone(),
        collect_url: collection_req.collect_url.clone(),
        collect_type: collection_req.collect_type,
        collect_mid: collection_req.collect_mid,
        collect_appid: collection_req.collect_appid.clone(),
        collect_appkey: collection_req.collect_appkey.clone(),
        collect_param: collection_req.collect_param.clone(),
        collect_filter: collection_req.collect_filter.clone(),
        collect_filter_from: collection_req.collect_filter_from.clone(),
        collect_opt: collection_req.collect_opt,
        collect_sync_pic_opt: collection_req.collect_sync_pic_opt,
        collect_remove_ad: collection_req.collect_remove_ad,
        collect_convert_webp: collection_req.collect_convert_webp,
        collect_download_retry: collection_req.collect_download_retry,
        collect_status: collection_req.collect_status,
        created_at: mongodb::bson::DateTime::now(),
        updated_at: mongodb::bson::DateTime::now(),
    };

    match collection.insert_one(new_collection, None).await {
        Ok(_) => {
            HttpResponse::Created().json(json!({"success": true, "message": "Collection created"}))
        }
        Err(e) => {
            eprintln!("Failed to create collection: {}", e);
            HttpResponse::InternalServerError()
                .json(json!({"success": false, "message": "Failed to create collection"}))
        }
    }
}

// POST /api/admin/collections/{id}/collect
pub async fn start_collection_collect(
    path: web::Path<String>,
    db: web::Data<Database>,
    collect_req: Option<web::Json<CollectRequest>>,
    session: Session,
) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }

    let collection_id = match mongodb::bson::oid::ObjectId::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest()
                .json(json!({"success": false, "message": "Invalid collection ID"}))
        }
    };

    // 获取采集源配置
    let collection = match db
        .collection::<Collection>("collections")
        .find_one(doc! {"_id": collection_id}, None)
        .await
    {
        Ok(Some(c)) => c,
        Ok(None) => {
            return HttpResponse::NotFound()
                .json(json!({"success": false, "message": "Collection not found"}))
        }
        Err(e) => {
            eprintln!("Failed to fetch collection: {}", e);
            return HttpResponse::InternalServerError()
                .json(json!({"success": false, "message": "Failed to fetch collection"}));
        }
    };

    // 检查是否有绑定的分类
    let bindings_collection = db.collection::<Binding>("bindings");
    let bindings_count = match bindings_collection
        .count_documents(doc! {"source_flag": &collection.collect_name}, None)
        .await
    {
        Ok(count) => count,
        Err(e) => {
            eprintln!("Failed to count bindings: {}", e);
            return HttpResponse::InternalServerError()
                .json(json!({"success": false, "message": "Failed to check bindings"}));
        }
    };

    // 如果没有绑定分类，返回错误
    if bindings_count == 0 {
        return HttpResponse::Ok().json(json!({
            "success": false,
            "message": "请先绑定分类",
            "needs_binding": true
        }));
    }

    // 解析hours参数
    let hours_text = collect_req
        .as_ref()
        .and_then(|req| req.hours)
        .map(|h| format!("采集任务已启动 ({}小时内)", h))
        .unwrap_or_else(|| "采集任务已启动 (全部数据)".to_string());

    let hours_param = collect_req
        .as_ref()
        .and_then(|req| req.hours)
        .map(|h| h.to_string());

    // 生成任务ID
    let task_id = uuid::Uuid::new_v4().to_string();
    let task_id_clone = task_id.clone();

    // 启动后台采集任务
    tokio::spawn(async move {
        if let Err(e) = crate::collect_handlers::start_batch_collect(
            &db,
            collection,
            hours_param,
            task_id_clone,
        )
        .await
        {
            eprintln!("Batch collect failed: {}", e);
        }
    });

    HttpResponse::Ok().json(json!({
        "success": true,
        "message": hours_text,
        "task_id": task_id,
        "total_pages": 1 // 将在实际采集中更新
    }))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectRequest {
    pub hours: Option<i32>,
}

// PUT /api/admin/collections/{id}
pub async fn update_collection(
    path: web::Path<String>,
    db: web::Data<Database>,
    collection_req: web::Json<CollectionRequest>,
    session: Session,
) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }
    let collection = db.collection::<Collection>("collections");
    let collection_id = match mongodb::bson::oid::ObjectId::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().body("Invalid collection ID"),
    };

    let update_doc = doc! {
        "$set": {
            "collect_name": &collection_req.collect_name,
            "collect_url": &collection_req.collect_url,
            "collect_type": collection_req.collect_type,
            "collect_mid": collection_req.collect_mid,
            "collect_appid": &collection_req.collect_appid,
            "collect_appkey": &collection_req.collect_appkey,
            "collect_param": &collection_req.collect_param,
            "collect_filter": &collection_req.collect_filter,
            "collect_filter_from": &collection_req.collect_filter_from,
            "collect_opt": collection_req.collect_opt,
            "collect_sync_pic_opt": collection_req.collect_sync_pic_opt,
            "collect_remove_ad": collection_req.collect_remove_ad,
            "collect_convert_webp": collection_req.collect_convert_webp,
            "collect_download_retry": collection_req.collect_download_retry,
            "collect_status": collection_req.collect_status,
            "updated_at": mongodb::bson::DateTime::now(),
        }
    };

    match collection
        .update_one(doc! {"_id": collection_id}, update_doc, None)
        .await
    {
        Ok(result) => {
            if result.matched_count > 0 {
                HttpResponse::Ok()
                    .json(json!({"success": true, "message": "Collection updated successfully"}))
            } else {
                HttpResponse::NotFound()
                    .json(json!({"success": false, "message": "Collection not found"}))
            }
        }
        Err(e) => {
            eprintln!("Failed to update collection: {}", e);
            HttpResponse::InternalServerError()
                .json(json!({"success": false, "message": "Failed to update collection"}))
        }
    }
}

// GET /api/admin/collect/progress/{task_id}
pub async fn get_collect_progress(path: web::Path<String>, session: Session) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }

    let task_id = path.into_inner();

    // 从内存中获取任务进度（简化版本）
    let progress = crate::collect_handlers::get_task_progress(&task_id)
        .await
        .unwrap_or(crate::collect_handlers::CollectProgress {
            status: "not_found".to_string(),
            current_page: 0,
            total_pages: 0,
            success: 0,
            failed: 0,
            log: "任务不存在".to_string(),
        });

    HttpResponse::Ok().json(json!({
        "success": true,
        "progress": progress
    }))
}

// GET /api/admin/collect/running-tasks
pub async fn get_running_tasks(session: Session) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }

    // 获取所有运行中的任务（从collect_handlers中的全局存储获取）
    let tasks = crate::collect_handlers::get_all_running_tasks().await;

    HttpResponse::Ok().json(json!({
        "success": true,
        "tasks": tasks
    }))
}

// POST /api/admin/collect/stop/{task_id}
pub async fn stop_collect_task(path: web::Path<String>, session: Session) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }

    let task_id = path.into_inner();

    // 调用collect_handlers中的停止任务函数
    let stopped = crate::collect_handlers::stop_task(&task_id).await;

    if stopped {
        HttpResponse::Ok().json(json!({
            "success": true,
            "message": "任务已成功停止"
        }))
    } else {
        HttpResponse::NotFound().json(json!({
            "success": false,
            "message": "任务不存在或已经停止"
        }))
    }
}

// DELETE /api/admin/collections/{id}
pub async fn delete_collection(
    path: web::Path<String>,
    db: web::Data<Database>,
    session: Session,
) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }
    let collection = db.collection::<Collection>("collections");
    let collection_id = match mongodb::bson::oid::ObjectId::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().body("Invalid collection ID"),
    };

    match collection
        .delete_one(doc! {"_id": collection_id}, None)
        .await
    {
        Ok(result) => {
            if result.deleted_count > 0 {
                HttpResponse::Ok()
                    .json(json!({"success": true, "message": "Collection deleted successfully"}))
            } else {
                HttpResponse::NotFound()
                    .json(json!({"success": false, "message": "Collection not found"}))
            }
        }
        Err(e) => {
            eprintln!("Failed to delete collection: {}", e);
            HttpResponse::InternalServerError()
                .json(json!({"success": false, "message": "Failed to delete collection"}))
        }
    }
}

// --- Video Management API ---

#[derive(Debug, Deserialize)]
pub struct VodsQuery {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub type_id: Option<i32>,
    pub status: Option<i32>,
    pub search: Option<String>,
}

// GET /api/admin/vods
pub async fn get_vods_admin(
    db: web::Data<Database>,
    query: web::Query<VodsQuery>,
    session: Session,
) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }

    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).min(100);
    let skip = (page - 1) * limit;

    // 构建查询条件
    let mut filter_doc = doc! {};

    // 分类筛选
    if let Some(type_id) = query.type_id {
        filter_doc.insert("type_id", type_id);
    }

    // 状态筛选
    if let Some(status) = query.status {
        filter_doc.insert("vod_status", status);
    }

    // 搜索功能
    if let Some(search_term) = &query.search {
        if !search_term.trim().is_empty() {
            filter_doc.insert("vod_name", doc! {"$regex": search_term, "$options": "i"});
        }
    }

    let collection = db.collection::<Vod>("vods");
    let find_options = FindOptions::builder()
        .sort(doc! {"vod_pubdate": -1})
        .skip(skip as u64)
        .limit(limit as i64)
        .build();

    // 获取总数
    let total = match collection.count_documents(filter_doc.clone(), None).await {
        Ok(count) => count,
        Err(e) => {
            eprintln!("Failed to count vods: {}", e);
            return HttpResponse::InternalServerError().json(json!({
                "code": 0,
                "msg": "Failed to count videos",
                "page": page,
                "limit": limit,
                "total": 0,
                "videos": []
            }));
        }
    };

    // 获取分页数据
    match collection.find(filter_doc, find_options).await {
        Ok(cursor) => {
            let vods: Vec<Vod> = cursor.try_collect().await.unwrap_or_else(|_| vec![]);

            HttpResponse::Ok().json(json!({
                "code": 1,
                "msg": "success",
                "page": page,
                "limit": limit,
                "total": total,
                "videos": vods
            }))
        }
        Err(e) => {
            eprintln!("Failed to fetch vods: {}", e);
            HttpResponse::InternalServerError().json(json!({
                "code": 0,
                "msg": "Failed to fetch videos",
                "page": page,
                "limit": limit,
                "total": 0,
                "videos": []
            }))
        }
    }
}

// POST /api/admin/vods
pub async fn create_vod(
    db: web::Data<Database>,
    vod_req: web::Json<VodRequest>,
    session: Session,
) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }
    let collection = db.collection::<Vod>("vods");

    let new_vod = Vod {
        id: None,
        vod_name: vod_req.vod_name.clone(),
        type_id: vod_req.type_id,
        vod_status: vod_req.vod_status,
        vod_class: vod_req.vod_class.clone(),
        vod_pic: vod_req.vod_pic.clone(),
        vod_actor: vod_req.vod_actor.clone(),
        vod_director: vod_req.vod_director.clone(),
        vod_remarks: vod_req.vod_remarks.clone(),
        vod_pubdate: mongodb::bson::DateTime::now(),
        vod_area: vod_req.vod_area.clone(),
        vod_lang: vod_req.vod_lang.clone(),
        vod_year: vod_req.vod_year.clone(),
        vod_content: vod_req.vod_content.clone(),
        vod_hits: Some(0),
        vod_hits_day: Some(0),
        vod_hits_week: Some(0),
        vod_hits_month: Some(0),
        vod_score: Some("0.0".to_string()),
        vod_play_urls: vec![], // Empty initially
    };

    match collection.insert_one(new_vod, None).await {
        Ok(_) => HttpResponse::Created().json(json!({
            "success": true,
            "message": "Video created successfully"
        })),
        Err(e) => {
            eprintln!("Failed to create video: {}", e);
            HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to create video"
            }))
        }
    }
}

// PUT /api/admin/vods/{id}
pub async fn update_vod(
    path: web::Path<String>,
    db: web::Data<Database>,
    vod_req: web::Json<VodRequest>,
    session: Session,
) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }
    let collection = db.collection::<Vod>("vods");
    let vod_id = match mongodb::bson::oid::ObjectId::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().body("Invalid video ID"),
    };

    let update_doc = doc! {
        "$set": {
            "vod_name": &vod_req.vod_name,
            "type_id": vod_req.type_id,
            "vod_status": vod_req.vod_status,
            "vod_class": &vod_req.vod_class,
            "vod_pic": &vod_req.vod_pic,
            "vod_actor": &vod_req.vod_actor,
            "vod_director": &vod_req.vod_director,
            "vod_remarks": &vod_req.vod_remarks,
            "vod_area": &vod_req.vod_area,
            "vod_lang": &vod_req.vod_lang,
            "vod_year": &vod_req.vod_year,
            "vod_content": &vod_req.vod_content,
        }
    };

    match collection
        .update_one(doc! {"_id": vod_id}, update_doc, None)
        .await
    {
        Ok(result) => {
            if result.matched_count > 0 {
                HttpResponse::Ok().json(json!({
                    "success": true,
                    "message": "Video updated successfully"
                }))
            } else {
                HttpResponse::NotFound().json(json!({
                    "success": false,
                    "message": "Video not found"
                }))
            }
        }
        Err(e) => {
            eprintln!("Failed to update video: {}", e);
            HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to update video"
            }))
        }
    }
}

// DELETE /api/admin/vods/{id}
pub async fn delete_vod(
    path: web::Path<String>,
    db: web::Data<Database>,
    session: Session,
) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }
    let collection = db.collection::<Vod>("vods");
    let vod_id = match mongodb::bson::oid::ObjectId::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().body("Invalid video ID"),
    };

    match collection.delete_one(doc! {"_id": vod_id}, None).await {
        Ok(result) => {
            if result.deleted_count > 0 {
                HttpResponse::Ok()
                    .json(json!({"success": true, "message": "Video deleted successfully"}))
            } else {
                HttpResponse::NotFound()
                    .json(json!({"success": false, "message": "Video not found"}))
            }
        }
        Err(e) => {
            eprintln!("Failed to delete video: {}", e);
            HttpResponse::InternalServerError()
                .json(json!({"success": false, "message": "Failed to delete video"}))
        }
    }
}

// DELETE /api/admin/vods/batch
pub async fn batch_delete_vods(
    db: web::Data<Database>,
    batch_req: web::Json<BatchDeleteRequest>,
    session: Session,
) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }

    let collection = db.collection::<Vod>("vods");
    let mut object_ids = Vec::new();
    let mut invalid_ids = Vec::new();

    // Parse all IDs and separate valid from invalid
    for id_str in &batch_req.ids {
        match mongodb::bson::oid::ObjectId::parse_str(id_str) {
            Ok(id) => object_ids.push(id),
            Err(_) => invalid_ids.push(id_str.clone()),
        }
    }

    if object_ids.is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "success": false,
            "message": "No valid video IDs provided",
            "invalid_ids": invalid_ids
        }));
    }

    // Delete all valid videos
    match collection
        .delete_many(doc! {"_id": {"$in": object_ids}}, None)
        .await
    {
        Ok(result) => {
            let response = json!({
                "success": true,
                "message": "Videos deleted successfully",
                "deleted_count": result.deleted_count,
                "invalid_ids": invalid_ids.len(),
                "invalid_id_list": invalid_ids
            });
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            eprintln!("Failed to batch delete videos: {}", e);
            HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to delete videos",
                "error": e.to_string()
            }))
        }
    }
}

// --- Website Configuration Management API ---

// GET /api/admin/configs
pub async fn get_configs(db: web::Data<Database>, session: Session) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }
    let collection = db.collection::<Config>("configs");
    let find_options = FindOptions::builder().sort(doc! {"config_sort": 1}).build();

    match collection.find(None, find_options).await {
        Ok(cursor) => {
            let configs: Vec<Config> = cursor.try_collect().await.unwrap_or_else(|_| vec![]);
            HttpResponse::Ok().json(configs)
        }
        Err(e) => {
            eprintln!("Failed to fetch configs: {}", e);
            HttpResponse::InternalServerError().body("Failed to fetch configs")
        }
    }
}

// GET /api/admin/configs/{key}
pub async fn get_config_by_key(
    path: web::Path<String>,
    db: web::Data<Database>,
    session: Session,
) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }
    let collection = db.collection::<Config>("configs");
    let config_key = path.into_inner();

    match collection
        .find_one(doc! {"config_key": &config_key}, None)
        .await
    {
        Ok(Some(config)) => HttpResponse::Ok().json(config),
        Ok(None) => HttpResponse::NotFound().body("Config not found"),
        Err(e) => {
            eprintln!("Failed to fetch config: {}", e);
            HttpResponse::InternalServerError().body("Failed to fetch config")
        }
    }
}

// POST /api/admin/configs
pub async fn create_config(
    db: web::Data<Database>,
    config_req: web::Json<ConfigRequest>,
    session: Session,
) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }
    let collection = db.collection::<Config>("configs");

    let new_config = Config {
        id: None,
        config_key: config_req.config_key.clone(),
        config_value: config_req.config_value.clone(),
        config_desc: config_req.config_desc.clone(),
        config_type: config_req.config_type.clone(),
        config_group: config_req.config_group.clone(),
        config_sort: config_req.config_sort,
        updated_at: mongodb::bson::DateTime::now(),
    };

    match collection.insert_one(new_config, None).await {
        Ok(_) => {
            HttpResponse::Created().json(json!({"success": true, "message": "Config created"}))
        }
        Err(e) => {
            if e.to_string().contains("E11000 duplicate key error") {
                HttpResponse::Conflict()
                    .json(json!({"success": false, "message": "Config key already exists"}))
            } else {
                eprintln!("Failed to create config: {}", e);
                HttpResponse::InternalServerError()
                    .json(json!({"success": false, "message": "Failed to create config"}))
            }
        }
    }
}

// PUT /api/admin/configs/{key}
pub async fn update_config(
    path: web::Path<String>,
    db: web::Data<Database>,
    config_req: web::Json<ConfigRequest>,
    session: Session,
) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }
    let collection = db.collection::<Config>("configs");
    let config_key = path.into_inner();

    let update_doc = doc! {
        "$set": {
            "config_value": &config_req.config_value,
            "config_desc": &config_req.config_desc,
            "config_type": &config_req.config_type,
            "config_group": &config_req.config_group,
            "config_sort": config_req.config_sort,
            "updated_at": mongodb::bson::DateTime::now(),
        }
    };

    match collection
        .update_one(doc! {"config_key": &config_key}, update_doc, None)
        .await
    {
        Ok(result) => {
            if result.matched_count > 0 {
                HttpResponse::Ok()
                    .json(json!({"success": true, "message": "Config updated successfully"}))
            } else {
                HttpResponse::NotFound()
                    .json(json!({"success": false, "message": "Config not found"}))
            }
        }
        Err(e) => {
            eprintln!("Failed to update config: {}", e);
            HttpResponse::InternalServerError()
                .json(json!({"success": false, "message": "Failed to update config"}))
        }
    }
}

// DELETE /api/admin/configs/{key}
pub async fn delete_config(
    path: web::Path<String>,
    db: web::Data<Database>,
    session: Session,
) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }
    let collection = db.collection::<Config>("configs");
    let config_key = path.into_inner();

    match collection
        .delete_one(doc! {"config_key": &config_key}, None)
        .await
    {
        Ok(result) => {
            if result.deleted_count > 0 {
                HttpResponse::Ok()
                    .json(json!({"success": true, "message": "Config deleted successfully"}))
            } else {
                HttpResponse::NotFound()
                    .json(json!({"success": false, "message": "Config not found"}))
            }
        }
        Err(e) => {
            eprintln!("Failed to delete config: {}", e);
            HttpResponse::InternalServerError()
                .json(json!({"success": false, "message": "Failed to delete config"}))
        }
    }
}

// POST /api/admin/types
pub async fn create_type(
    db: web::Data<Database>,
    type_req: web::Json<TypeRequest>,
    session: Session,
) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }
    let collection = db.collection::<Type>("types");

    // In a real system, you'd generate type_id and handle type_mid, etc.
    // For simplicity, let's assume type_id is auto-incremented or managed externally for now.
    // Or, query max type_id and increment.
    let new_type_id = match collection
        .find_one(
            None,
            FindOneOptions::builder().sort(doc! {"type_id": -1}).build(),
        )
        .await
    {
        Ok(Some(last_type)) => last_type.type_id + 1,
        _ => 1, // Start from 1 if no types exist
    };

    let new_type = Type {
        id: None, // MongoDB will generate ObjectId
        type_id: new_type_id,
        type_name: type_req.type_name.clone(),
        type_pid: type_req.type_pid,
        type_en: type_req.type_en.clone(),
        type_sort: type_req.type_sort.unwrap_or(0),
        type_status: type_req.type_status.unwrap_or(1),
        type_mid: type_req.type_mid,
        type_key: type_req.type_key.clone(),
        type_des: type_req.type_des.clone(),
        type_title: type_req.type_title.clone(),
        type_tpl: None,
        type_tpl_list: None,
        type_tpl_detail: None,
        type_tpl_play: None,
        type_tpl_down: None,
        subarea: type_req.subarea.clone(),
        subyear: type_req.subyear.clone(),
    };

    match collection.insert_one(new_type, None).await {
        Ok(_) => HttpResponse::Created().json(json!({"success": true, "message": "Type created"})),
        Err(e) => {
            eprintln!("Failed to create type: {}", e);
            HttpResponse::InternalServerError()
                .json(json!({"success": false, "message": "Failed to create type"}))
        }
    }
}

// PUT /api/admin/types/{id}
pub async fn update_type(
    path: web::Path<String>,
    db: web::Data<Database>,
    type_req: web::Json<TypeRequest>,
    session: Session,
) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }
    let collection = db.collection::<Type>("types");
    let type_id: i32 = match path.into_inner().parse() {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest()
                .json(json!({"success": false, "message": "Invalid type ID"}))
        }
    };

    let mut update_fields = doc! {
        "type_name": &type_req.type_name,
        "type_pid": type_req.type_pid,
    };

    if let Some(ref type_en) = type_req.type_en {
        update_fields.insert("type_en", type_en);
    }
    if let Some(type_sort) = type_req.type_sort {
        update_fields.insert("type_sort", type_sort);
    }
    if let Some(type_status) = type_req.type_status {
        update_fields.insert("type_status", type_status);
    }
    if let Some(type_mid) = type_req.type_mid {
        update_fields.insert("type_mid", type_mid);
    }
    if let Some(ref type_key) = type_req.type_key {
        update_fields.insert("type_key", type_key);
    }
    if let Some(ref type_des) = type_req.type_des {
        update_fields.insert("type_des", type_des);
    }
    if let Some(ref type_title) = type_req.type_title {
        update_fields.insert("type_title", type_title);
    }
    if let Some(ref subarea) = type_req.subarea {
        update_fields.insert("subarea", subarea);
    }
    if let Some(ref subyear) = type_req.subyear {
        update_fields.insert("subyear", subyear);
    }

    let update_doc = doc! {
        "$set": update_fields
    };

    match collection
        .update_one(doc! {"type_id": type_id}, update_doc, None)
        .await
    {
        Ok(result) => {
            if result.matched_count > 0 {
                HttpResponse::Ok()
                    .json(json!({"success": true, "message": "Type updated successfully"}))
            } else {
                HttpResponse::NotFound()
                    .json(json!({"success": false, "message": "Type not found"}))
            }
        }
        Err(e) => {
            eprintln!("Failed to update type: {}", e);
            HttpResponse::InternalServerError()
                .json(json!({"success": false, "message": "Failed to update type"}))
        }
    }
}

// DELETE /api/admin/types/{id}
pub async fn delete_type(
    path: web::Path<String>,
    db: web::Data<Database>,
    session: Session,
) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }
    let collection = db.collection::<Type>("types");
    let type_id: i32 = match path.into_inner().parse() {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest()
                .json(json!({"success": false, "message": "Invalid type ID"}))
        }
    };

    match collection.delete_one(doc! {"type_id": type_id}, None).await {
        Ok(result) => {
            if result.deleted_count > 0 {
                HttpResponse::Ok()
                    .json(json!({"success": true, "message": "Type deleted successfully"}))
            } else {
                HttpResponse::NotFound()
                    .json(json!({"success": false, "message": "Type not found"}))
            }
        }
        Err(e) => {
            eprintln!("Failed to delete type: {}", e);
            HttpResponse::InternalServerError()
                .json(json!({"success": false, "message": "Failed to delete type"}))
        }
    }
}

// --- Binding Management API ---
// DELETE /api/admin/bindings/{id}
pub async fn delete_binding(
    db: web::Data<Database>,
    session: Session,
    path: web::Path<String>,
) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }
    let collection = db.collection::<Binding>("bindings");
    let binding_id = path.into_inner();

    match collection.delete_one(doc! {"_id": binding_id}, None).await {
        Ok(result) => {
            if result.deleted_count > 0 {
                HttpResponse::Ok()
                    .json(json!({"success": true, "message": "Binding deleted successfully"}))
            } else {
                HttpResponse::NotFound()
                    .json(json!({"success": false, "message": "Binding not found"}))
            }
        }
        Err(e) => {
            eprintln!("Failed to delete binding: {}", e);
            HttpResponse::InternalServerError()
                .json(json!({"success": false, "message": "Failed to delete binding"}))
        }
    }
}
// GET /api/admin/bindings
pub async fn get_bindings(db: web::Data<Database>, session: Session) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }
    let collection = db.collection::<Binding>("bindings");

    match collection.find(None, None).await {
        Ok(cursor) => {
            let bindings: Vec<Binding> = cursor.try_collect().await.unwrap_or_else(|_| vec![]);
            HttpResponse::Ok().json(bindings)
        }
        Err(e) => {
            eprintln!("Failed to fetch bindings: {}", e);
            HttpResponse::InternalServerError().body("Failed to fetch bindings")
        }
    }
}

// GET /api/admin/collections/{id}/binding-status
pub async fn get_collection_binding_status(
    path: web::Path<String>,
    db: web::Data<Database>,
    session: Session,
) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }

    let collection_id = match mongodb::bson::oid::ObjectId::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest()
                .json(json!({"success": false, "message": "Invalid collection ID"}))
        }
    };

    // 获取采集源配置
    let collection = match db
        .collection::<Collection>("collections")
        .find_one(doc! {"_id": collection_id}, None)
        .await
    {
        Ok(Some(c)) => c,
        Ok(None) => {
            return HttpResponse::NotFound()
                .json(json!({"success": false, "message": "Collection not found"}))
        }
        Err(e) => {
            eprintln!("Failed to fetch collection: {}", e);
            return HttpResponse::InternalServerError()
                .json(json!({"success": false, "message": "Failed to fetch collection"}));
        }
    };

    // 检查是否有绑定的分类
    let bindings_collection = db.collection::<Binding>("bindings");
    let bindings_count = match bindings_collection
        .count_documents(doc! {"source_flag": &collection.collect_name}, None)
        .await
    {
        Ok(count) => count,
        Err(e) => {
            eprintln!("Failed to count bindings: {}", e);
            return HttpResponse::InternalServerError()
                .json(json!({"success": false, "message": "Failed to check bindings"}));
        }
    };

    let has_bindings = bindings_count > 0;

    HttpResponse::Ok().json(json!({
        "success": true,
        "has_bindings": has_bindings,
        "bindings_count": bindings_count,
        "source_flag": collection.collect_name,
        "message": if has_bindings {
            format!("已绑定 {} 个分类", bindings_count)
        } else {
            "请先绑定分类".to_string()
        }
    }))
}

// POST /api/admin/bindings
pub async fn create_or_update_binding(
    db: web::Data<Database>,
    binding_req: web::Json<BindingRequest>,
    session: Session,
) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }
    let collection = db.collection::<Binding>("bindings");

    let binding_id = format!("{}_{}", binding_req.source_flag, binding_req.external_id);

    // Fetch local type name for the binding
    let type_collection = db.collection::<Type>("types");
    let local_type_name = match type_collection
        .find_one(doc! {"type_id": binding_req.local_type_id}, None)
        .await
    {
        Ok(Some(t)) => t.type_name,
        _ => "Unknown Type".to_string(), // Default if type not found
    };

    let now = mongodb::bson::DateTime::now();
    let new_binding = Binding {
        id: binding_id.clone(),
        source_flag: binding_req.source_flag.clone(),
        external_id: binding_req.external_id.clone(),
        local_type_id: binding_req.local_type_id,
        local_type_name: local_type_name.clone(),
        created_at: now,
        updated_at: now,
    };

    match collection.insert_one(new_binding, None).await {
        Ok(_) => HttpResponse::Created()
            .json(json!({"success": true, "message": "Binding created/updated"})),
        Err(e) => {
            // If it's a duplicate key error, try to update instead (upsert behavior)
            if e.to_string().contains("E11000 duplicate key error") {
                let update_doc = doc! {"$set": {
                    "source_flag": &binding_req.source_flag,
                    "external_id": &binding_req.external_id,
                    "local_type_id": binding_req.local_type_id,
                    "local_type_name": local_type_name.clone(),
                    "updated_at": mongodb::bson::DateTime::now()
                }};
                match collection
                    .update_one(doc! {"_id": binding_id}, update_doc, None)
                    .await
                {
                    Ok(_) => HttpResponse::Ok()
                        .json(json!({"success": true, "message": "Binding updated"})),
                    Err(e) => {
                        eprintln!("Failed to update binding: {}", e);
                        HttpResponse::InternalServerError()
                            .json(json!({"success": false, "message": "Failed to update binding"}))
                    }
                }
            } else {
                eprintln!("Failed to create binding: {}", e);
                HttpResponse::InternalServerError()
                    .json(json!({"success": false, "message": "Failed to create binding"}))
            }
        }
    }
}

// --- Index Management API ---

// POST /api/admin/indexes/create
pub async fn create_indexes(db: web::Data<Database>, session: Session) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }

    let index_manager = IndexManager::new(db.get_ref().clone());

    match index_manager.create_all_indexes().await {
        Ok(_) => HttpResponse::Ok().json(json!({
            "success": true,
            "message": "索引创建完成"
        })),
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "success": false,
            "message": format!("索引创建失败: {}", e)
        })),
    }
}

// GET /api/admin/indexes/status
pub async fn get_index_status(db: web::Data<Database>, session: Session) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }

    let index_manager = IndexManager::new(db.get_ref().clone());

    match index_manager.verify_indexes().await {
        Ok(_) => HttpResponse::Ok().json(json!({
            "success": true,
            "message": "所有索引状态正常"
        })),
        Err(e) => HttpResponse::Ok().json(json!({
            "success": false,
            "message": format!("索引验证失败: {}", e)
        })),
    }
}

// GET /api/admin/indexes/list
pub async fn list_indexes(db: web::Data<Database>, session: Session) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }

    let index_manager = IndexManager::new(db.get_ref().clone());

    // 使用IndexManager的show_index_status方法获取索引信息
    match index_manager.show_index_status().await {
        Ok(_) => {
            // 返回简单的成功响应，详细状态在控制台输出
            HttpResponse::Ok().json(json!({
                "success": true,
                "message": "索引状态已输出到控制台"
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "success": false,
            "message": format!("获取索引状态失败: {}", e)
        })),
    }
}

// GET /api/admin/indexes/data
pub async fn get_indexes_data(db: web::Data<Database>, session: Session) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }

    let index_manager = IndexManager::new(db.get_ref().clone());
    match index_manager.get_all_indexes().await {
        Ok(indexes) => HttpResponse::Ok().json(json!({
            "success": true,
            "data": indexes
        })),
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "success": false,
            "message": format!("获取索引数据失败: {}", e)
        })),
    }
}

// GET /api/admin/statistics
pub async fn get_statistics(db: web::Data<Database>, session: Session) -> impl Responder {
    if let Err(response) = check_auth(&session) {
        return response;
    }

    let mut stats = json!({
        "success": true,
        "data": {
            "vods": 0,
            "types": 0,
            "collections": 0,
            "bindings": 0,
            "configs": 0,
            "users": 0
        }
    });

    // 获取视频数量
    if let Ok(count) = db
        .collection::<mongodb::bson::Document>("vods")
        .count_documents(None, None)
        .await
    {
        stats["data"]["vods"] = count.into();
    }

    // 获取分类数量
    if let Ok(count) = db
        .collection::<mongodb::bson::Document>("types")
        .count_documents(None, None)
        .await
    {
        stats["data"]["types"] = count.into();
    }

    // 获取采集源数量
    if let Ok(count) = db
        .collection::<mongodb::bson::Document>("collections")
        .count_documents(None, None)
        .await
    {
        stats["data"]["collections"] = count.into();
    }

    // 获取绑定数量
    if let Ok(count) = db
        .collection::<mongodb::bson::Document>("bindings")
        .count_documents(None, None)
        .await
    {
        stats["data"]["bindings"] = count.into();
    }

    // 获取配置数量
    if let Ok(count) = db
        .collection::<mongodb::bson::Document>("configs")
        .count_documents(None, None)
        .await
    {
        stats["data"]["configs"] = count.into();
    }

    // 获取用户数量
    if let Ok(count) = db
        .collection::<mongodb::bson::Document>("users")
        .count_documents(None, None)
        .await
    {
        stats["data"]["users"] = count.into();
    }

    HttpResponse::Ok().json(stats)
}
