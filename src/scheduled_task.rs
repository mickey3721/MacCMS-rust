use mongodb::{Database, Collection as MongoCollection};
use mongodb::bson::{doc, oid::ObjectId, DateTime};
use mongodb::options::FindOptions;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;
use std::sync::Arc;
use chrono::{DateTime as ChronoDateTime};
use tokio::time::{sleep, interval};
use futures::TryStreamExt;
use crate::models::Collection;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScheduledTaskConfig {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub enabled: bool,
    pub interval_hours: i32,
    pub last_run: Option<DateTime>,
    pub next_run: Option<DateTime>,
    pub running_collections: Vec<String>, // 正在运行的采集源ID列表
    pub current_collection_index: usize, // 当前正在执行的采集源索引
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TaskExecutionLog {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub task_id: String,
    pub collection_id: String,
    pub collection_name: String,
    pub status: String, // "running", "completed", "failed"
    pub started_at: DateTime,
    pub completed_at: Option<DateTime>,
    pub message: Option<String>,
    pub videos_collected: Option<i32>,
    pub errors: Option<String>,
}

pub struct ScheduledTaskManager {
    db: Database,
    config_collection: MongoCollection<ScheduledTaskConfig>,
    log_collection: MongoCollection<TaskExecutionLog>,
    is_running: Arc<RwLock<bool>>,
    current_task: Arc<RwLock<Option<String>>>,
}

impl ScheduledTaskManager {
    pub fn new(db: Database) -> Self {
        let config_collection = db.collection::<ScheduledTaskConfig>("scheduled_task_configs");
        let log_collection = db.collection::<TaskExecutionLog>("task_execution_logs");
        
        Self {
            db,
            config_collection,
            log_collection,
            is_running: Arc::new(RwLock::new(false)),
            current_task: Arc::new(RwLock::new(None)),
        }
    }

    /// 初始化定时任务配置
    pub async fn initialize_config(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 检查是否已存在配置
        let existing_config = self.config_collection.find_one(doc! {}, None).await?;
        
        if existing_config.is_none() {
            // 创建默认配置
            let now = DateTime::now();
            let next_run_millis = now.timestamp_millis() + (12 * 3600 * 1000);
            let next_run = DateTime::from_millis(next_run_millis);
            
            let config = ScheduledTaskConfig {
                id: None,
                enabled: false,
                interval_hours: 12,
                last_run: None,
                next_run: Some(next_run),
                running_collections: Vec::new(),
                current_collection_index: 0,
                created_at: now,
                updated_at: now,
            };
            
            self.config_collection.insert_one(&config, None).await?;
            println!("✅ 定时任务配置初始化完成");
        }
        
        Ok(())
    }

    /// 获取当前配置
    pub async fn get_config(&self) -> Result<Option<ScheduledTaskConfig>, Box<dyn std::error::Error + Send + Sync>> {
        let config = self.config_collection.find_one(doc! {}, None).await?;
        Ok(config)
    }

    /// 更新配置
    pub async fn update_config(&self, enabled: bool, interval_hours: Option<i32>) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let now = DateTime::now();
        let next_run = if enabled {
            let next_run_millis = now.timestamp_millis() + ((interval_hours.unwrap_or(12) as i64) * 3600 * 1000);
            Some(DateTime::from_millis(next_run_millis))
        } else {
            None
        };

        let update = doc! {
            "$set": {
                "enabled": enabled,
                "interval_hours": interval_hours.unwrap_or(12),
                "next_run": next_run,
                "updated_at": now,
                "running_collections": [],
                "current_collection_index": 0
            }
        };

        let result = self.config_collection.update_one(doc! {}, update, None).await?;
        Ok(result.modified_count > 0)
    }

    /// 启动定时任务
    pub async fn start_scheduled_task(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 检查配置是否已经启用
        {
            let is_running = self.is_running.read().await;
            if let Some(config) = self.get_config().await? {
                if config.enabled && *is_running {
                    return Ok(());
                }
            }
        }

        // 步骤1：立即设置当前任务状态，确保前端能立即看到"运行中"状态
        println!("🔍 步骤1：立即设置任务运行状态...");
        let immediate_task_id = ObjectId::new().to_hex();
        {
            let mut current_task = self.current_task.write().await;
            *current_task = Some(immediate_task_id.clone());
        }
        
        // 步骤2：更新配置为启用状态
        println!("🔍 步骤2：更新配置为启用状态...");
        self.update_config(true, None).await?;
        
        // 步骤3：设置内存运行状态
        println!("🔍 步骤3：设置内存运行状态...");
        {
            let mut is_running = self.is_running.write().await;
            *is_running = true;
        }
        println!("🚀 定时采集任务已启动");

        // 步骤4：启动定时任务循环（异步执行，不阻塞当前流程）
        println!("🔍 步骤4：启动定时任务循环...");
        let db = self.db.clone();
        let is_running_clone = self.is_running.clone();
        let current_task_clone = self.current_task.clone();
        
        tokio::spawn(async move {
            let manager = ScheduledTaskManager::new(db);
            manager.run_scheduled_task_loop(is_running_clone, current_task_clone).await;
        });

        // 步骤5：验证状态更新（确保前端能看到运行状态）
        println!("🔍 步骤5：验证状态更新...");
        let task_is_set = {
            let current_task = self.current_task.read().await;
            current_task.is_some()
        };
        
        let is_running_status = {
            let is_running_guard = self.is_running.read().await;
            *is_running_guard
        };
        println!("🔍 任务状态设置结果: {}, 内存运行状态: {}", task_is_set, is_running_status);
        println!("✅ 步骤5验证完成，继续执行后续步骤...");

        // 立即执行一次采集任务
        println!("🔄 立即执行一次采集任务...");
        
        // 步骤6：检查是否有启用的采集源
        println!("🔍 步骤6：检查启用的采集源...");
        let collections_collection = self.db.collection::<Collection>("collections");
        let filter = doc! { "collect_status": 1 };
        let enabled_collections_count = match collections_collection.count_documents(filter.clone(), None).await {
            Ok(count) => {
                println!("🔍 找到 {} 个启用的采集源", count);
                count
            }
            Err(e) => {
                eprintln!("❌ 查询采集源失败: {}", e);
                // 即使查询失败，也要清除任务状态
                *self.current_task.write().await = None;
                return Ok(());
            }
        };
        
        if enabled_collections_count == 0 {
            println!("⚠️ 没有启用的采集源，跳过立即执行");
            // 清除任务状态
            *self.current_task.write().await = None;
            return Ok(());
        }
        
        // 步骤7：获取配置
        println!("🔍 步骤7：获取定时任务配置...");
        let config = match self.get_config().await {
            Ok(Some(config)) => {
                println!("🔍 获取配置成功，启用状态: {}", config.enabled);
                config
            }
            Ok(None) => {
                println!("⚠️ 没有找到定时任务配置");
                // 清除任务状态
                *self.current_task.write().await = None;
                return Ok(());
            }
            Err(e) => {
                eprintln!("❌ 获取配置失败: {}", e);
                // 清除任务状态
                *self.current_task.write().await = None;
                return Ok(());
            }
        };
        
        // 步骤8：执行立即采集任务
        println!("🔍 步骤8：执行立即采集任务...");
        match self.execute_immediate_collection(&config).await {
            Ok(_) => {
                println!("✅ 立即执行采集任务完成");
            }
            Err(e) => {
                eprintln!("❌ 立即执行采集任务失败: {}", e);
                println!("错误详情: {:?}", e);
            }
        }
        
        // 步骤9：清除当前任务状态
        println!("🔍 步骤9：清除任务运行状态...");
        *self.current_task.write().await = None;

        Ok(())
    }

    /// 停止定时任务
    pub async fn stop_scheduled_task(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut is_running = self.is_running.write().await;
        
        // 检查配置是否启用，如果配置已禁用则只需要更新内存状态
        if let Some(config) = self.get_config().await? {
            if !config.enabled {
                *is_running = false;
                return Ok(());
            }
        }
        
        // 无论内存状态如何，都要更新配置为禁用状态
        self.update_config(false, None).await?;
        
        // 更新内存状态
        *is_running = false;
        println!("🛑 定时采集任务已停止");

        Ok(())
    }

    /// 定时任务主循环
    async fn run_scheduled_task_loop(
        &self,
        is_running: Arc<RwLock<bool>>,
        current_task: Arc<RwLock<Option<String>>>,
    ) {
        let mut interval_timer = interval(tokio::time::Duration::from_secs(60)); // 每分钟检查一次

        loop {
            // 检查是否应该停止
            if !*is_running.read().await {
                break;
            }

            // 检查是否到了执行时间
            if let Ok(Some(config)) = self.get_config().await {
                if config.enabled {
                    if let Some(next_run) = config.next_run {
                        let now = ChronoDateTime::from_timestamp(DateTime::now().timestamp_millis() as i64 / 1000, 0).unwrap();
                        let next_run_time = ChronoDateTime::from_timestamp(next_run.timestamp_millis() as i64 / 1000, 0).unwrap();
                        
                        if now >= next_run_time {
                            // 执行采集任务
                            if let Err(e) = self.execute_scheduled_collection(&config).await {
                                eprintln!("❌ 执行定时采集任务失败: {}", e);
                            }
                        }
                    }
                }
            }

            interval_timer.tick().await;
        }
    }

    /// 执行立即采集任务（跳过运行状态检查）
    async fn execute_immediate_collection(&self, config: &ScheduledTaskConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("🔄 开始执行立即采集任务");

        // 确保任务状态已设置
        let current_task = self.current_task.read().await;
        if current_task.is_none() {
            println!("⚠️ 警告：当前任务状态未设置，设置默认任务ID");
            drop(current_task);
            let default_task_id = ObjectId::new().to_hex();
            *self.current_task.write().await = Some(default_task_id);
        } else {
            drop(current_task);
        }

        // 获取所有启用的采集源
        let collections_collection = self.db.collection::<Collection>("collections");
        let filter = doc! { "collect_status": 1 };
        let mut cursor = collections_collection.find(filter, None).await?;
        
        let mut collections: Vec<Collection> = Vec::new();
        while let Ok(Some(collection)) = cursor.try_next().await {
            collections.push(collection);
        }

        if collections.is_empty() {
            println!("⚠️ 没有找到启用的采集源");
            return Ok(());
        }

        // 按顺序执行采集任务
        let total_collections = collections.len();
        let mut total_videos_collected = 0;
        let mut successful_collections = 0;
        let mut failed_collections = 0;

        for (index, collection) in collections.iter().enumerate() {
            println!("📥 开始采集第 {}/{} 个采集源: {}", index + 1, total_collections, collection.collect_name);
            
            // 记录任务开始
            let task_id = ObjectId::new().to_hex();
            let log_entry = TaskExecutionLog {
                id: None,
                task_id: task_id.clone(),
                collection_id: collection.id.clone().unwrap_or(ObjectId::new()).to_hex(),
                collection_name: collection.collect_name.clone(),
                status: "running".to_string(),
                started_at: DateTime::now(),
                completed_at: None,
                message: Some(format!("开始采集 {}", collection.collect_name)),
                videos_collected: None,
                errors: None,
            };

            self.log_collection.insert_one(&log_entry, None).await?;
            
            // 检查是否已有任务ID，如果有则使用已有的（避免覆盖立即执行的任务ID）
            let current_task = self.current_task.read().await;
            let final_task_id = if current_task.is_some() {
                current_task.as_ref().unwrap().clone()
            } else {
                task_id.clone()
            };
            drop(current_task); // 释放读锁
            
            // 如果没有任务ID，则设置一个
            if self.current_task.read().await.is_none() {
                *self.current_task.write().await = Some(task_id.clone());
            }

            // 执行采集（这里需要调用实际的采集逻辑）
            match self.collect_videos_from_source(&collection).await {
                Ok(videos_collected) => {
                    total_videos_collected += videos_collected;
                    successful_collections += 1;
                    
                    // 更新日志为完成状态
                    let update = doc! {
                        "$set": {
                            "status": "completed",
                            "completed_at": DateTime::now(),
                            "videos_collected": videos_collected,
                            "message": Some(format!("采集完成，获取 {} 个视频", videos_collected))
                        }
                    };
                    self.log_collection.update_one(doc! { "task_id": &task_id }, update, None).await?;
                    
                    println!("✅ 采集完成: {} (获取 {} 个视频)", collection.collect_name, videos_collected);
                }
                Err(e) => {
                    failed_collections += 1;
                    
                    // 更新日志为失败状态
                    let update = doc! {
                        "$set": {
                            "status": "failed",
                            "completed_at": DateTime::now(),
                            "errors": Some(e.to_string()),
                            "message": Some(format!("采集失败: {}", e))
                        }
                    };
                    self.log_collection.update_one(doc! { "task_id": &task_id }, update, None).await?;
                    
                    eprintln!("❌ 采集失败: {} - {}", collection.collect_name, e);
                }
            }

            // 只有当前任务ID匹配时才清除（避免清除立即执行的任务ID）
            let current_task = self.current_task.read().await;
            if let Some(ref current_id) = *current_task {
                if current_id == &task_id {
                    drop(current_task);
                    *self.current_task.write().await = None;
                }
            }

            // 采集间隔，避免请求过于频繁
            sleep(tokio::time::Duration::from_secs(5)).await;
        }

        println!("🎉 立即采集任务完成: 成功 {}/{}, 共获取 {} 个视频", 
            successful_collections, total_collections, total_videos_collected);

        Ok(())
    }

    /// 执行定时采集任务
    async fn execute_scheduled_collection(&self, config: &ScheduledTaskConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("🔄 开始执行定时采集任务");

        // 获取所有启用的采集源
        let collections_collection = self.db.collection::<Collection>("collections");
        let filter = doc! { "collect_status": 1 };
        let mut cursor = collections_collection.find(filter, None).await?;
        
        let mut collections: Vec<Collection> = Vec::new();
        while let Ok(Some(collection)) = cursor.try_next().await {
            collections.push(collection);
        }

        if collections.is_empty() {
            println!("⚠️ 没有找到启用的采集源");
            println!("🔍 调试信息: 查询条件为 collect_status: 1");
            return Ok(());
        }

        // 按顺序执行采集任务
        let total_collections = collections.len();
        let mut total_videos_collected = 0;
        let mut successful_collections = 0;
        let mut failed_collections = 0;

        for (index, collection) in collections.iter().enumerate() {
            // 检查任务是否还在运行
            if !*self.is_running.read().await {
                println!("⏹️ 定时任务已停止，中断采集");
                break;
            }

            println!("📥 开始采集第 {}/{} 个采集源: {}", index + 1, total_collections, collection.collect_name);
            
            // 记录任务开始
            let task_id = ObjectId::new().to_hex();
            let log_entry = TaskExecutionLog {
                id: None,
                task_id: task_id.clone(),
                collection_id: collection.id.clone().unwrap_or(ObjectId::new()).to_hex(),
                collection_name: collection.collect_name.clone(),
                status: "running".to_string(),
                started_at: DateTime::now(),
                completed_at: None,
                message: Some(format!("开始采集 {}", collection.collect_name)),
                videos_collected: None,
                errors: None,
            };

            self.log_collection.insert_one(&log_entry, None).await?;
            
            // 检查是否已有任务ID，如果有则使用已有的（避免覆盖立即执行的任务ID）
            let current_task = self.current_task.read().await;
            let final_task_id = if current_task.is_some() {
                current_task.as_ref().unwrap().clone()
            } else {
                task_id.clone()
            };
            drop(current_task); // 释放读锁
            
            // 如果没有任务ID，则设置一个
            if self.current_task.read().await.is_none() {
                *self.current_task.write().await = Some(task_id.clone());
            }

            // 执行采集（这里需要调用实际的采集逻辑）
            match self.collect_videos_from_source(&collection).await {
                Ok(videos_collected) => {
                    total_videos_collected += videos_collected;
                    successful_collections += 1;
                    
                    // 更新日志为完成状态
                    let update = doc! {
                        "$set": {
                            "status": "completed",
                            "completed_at": DateTime::now(),
                            "videos_collected": videos_collected,
                            "message": Some(format!("采集完成，获取 {} 个视频", videos_collected))
                        }
                    };
                    self.log_collection.update_one(doc! { "task_id": &task_id }, update, None).await?;
                    
                    println!("✅ 采集完成: {} (获取 {} 个视频)", collection.collect_name, videos_collected);
                }
                Err(e) => {
                    failed_collections += 1;
                    
                    // 更新日志为失败状态
                    let update = doc! {
                        "$set": {
                            "status": "failed",
                            "completed_at": DateTime::now(),
                            "errors": Some(e.to_string()),
                            "message": Some(format!("采集失败: {}", e))
                        }
                    };
                    self.log_collection.update_one(doc! { "task_id": &task_id }, update, None).await?;
                    
                    eprintln!("❌ 采集失败: {} - {}", collection.collect_name, e);
                }
            }

            // 只有当前任务ID匹配时才清除（避免清除立即执行的任务ID）
            let current_task = self.current_task.read().await;
            if let Some(ref current_id) = *current_task {
                if current_id == &task_id {
                    drop(current_task);
                    *self.current_task.write().await = None;
                }
            }

            // 采集间隔，避免请求过于频繁
            sleep(tokio::time::Duration::from_secs(5)).await;
        }

        // 更新配置中的执行时间
        let now = DateTime::now();
        let next_run_millis = now.timestamp_millis() + ((config.interval_hours as i64) * 3600 * 1000);
        let next_run = DateTime::from_millis(next_run_millis);

        let update = doc! {
            "$set": {
                "last_run": now,
                "next_run": next_run,
                "updated_at": now
            }
        };
        self.config_collection.update_one(doc! {}, update, None).await?;

        println!("🎉 定时采集任务完成: 成功 {}/{}, 共获取 {} 个视频", 
            successful_collections, total_collections, total_videos_collected);

        Ok(())
    }

    /// 从指定采集源采集视频（调用真实的采集逻辑）
    async fn collect_videos_from_source(&self, collection: &Collection) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
        use crate::collect_handlers::start_batch_collect;
        
        println!("🔄 正在从采集源采集视频: {}", collection.collect_name);
        
        // 生成任务ID
        let task_id = ObjectId::new().to_hex();
        
        // 调用真实的批量采集函数，专门采集当天数据（24小时内）
        match start_batch_collect(&self.db, collection.clone(), Some("24".to_string()), task_id.clone()).await {
            Ok(_) => {
                // 获取采集结果
                let videos_collected = self.get_videos_collected_count(&task_id).await.unwrap_or(0);
                println!("✅ 采集完成: {} (获取 {} 个视频)", collection.collect_name, videos_collected);
                Ok(videos_collected)
            }
            Err(e) => {
                eprintln!("❌ 采集失败: {} - {}", collection.collect_name, e);
                Err(e)
            }
        }
    }
    
    /// 获取采集的视频数量
    async fn get_videos_collected_count(&self, task_id: &str) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
        use crate::collect_handlers::get_task_progress;
        
        if let Some(progress) = get_task_progress(task_id).await {
            Ok(progress.success as i32)
        } else {
            Ok(0)
        }
    }

    /// 获取任务状态
    pub async fn get_task_status(&self) -> Result<HashMap<String, serde_json::Value>, Box<dyn std::error::Error + Send + Sync>> {
        let mut status = HashMap::new();
        
        // 获取配置状态
        let config_enabled = if let Some(config) = self.get_config().await? {
            status.insert("enabled".to_string(), serde_json::Value::Bool(config.enabled));
            status.insert("interval_hours".to_string(), serde_json::Value::Number(serde_json::Number::from(config.interval_hours)));
            status.insert("last_run".to_string(), serde_json::Value::String(
                config.last_run.map_or("从未运行".to_string(), |dt| format!("{}", dt.timestamp_millis()))
            ));
            status.insert("next_run".to_string(), serde_json::Value::String(
                config.next_run.map_or("未设置".to_string(), |dt| format!("{}", dt.timestamp_millis()))
            ));
            config.enabled
        } else {
            false
        };

        // 获取当前运行状态：检查配置状态、内存状态和当前任务
        let memory_is_running = *self.is_running.read().await;
        let current_task = self.current_task.read().await;
        let has_active_task = current_task.is_some();
        drop(current_task); // 释放读锁
        
        // 判断任务是否正在运行：
        // 1. 配置已启用
        // 2. 内存状态为运行中 或者 有当前任务（说明正在立即执行）
        let is_running = config_enabled && (memory_is_running || has_active_task);
        status.insert("is_running".to_string(), serde_json::Value::Bool(is_running));
        
        // 添加调试信息
        println!("🔍 状态检查 - 配置启用: {}, 内存运行: {}, 有活跃任务: {}, 最终状态: {}", config_enabled, memory_is_running, has_active_task, is_running);
        
        // 获取当前任务
        let current_task = self.current_task.read().await;
        if let Some(task_id) = current_task.as_ref() {
            status.insert("current_task_id".to_string(), serde_json::Value::String(task_id.clone()));
            
            // 获取任务详情
            if let Some(log) = self.log_collection.find_one(doc! { "task_id": task_id }, None).await? {
                status.insert("current_collection".to_string(), serde_json::Value::String(log.collection_name));
                status.insert("current_status".to_string(), serde_json::Value::String(log.status));
                status.insert("task_started_at".to_string(), serde_json::Value::String(format!("{}", log.started_at.timestamp_millis())));
            }
        }

        // 获取最近的执行记录
        let mut logs = Vec::new();
        let find_options = mongodb::options::FindOptions::builder()
            .sort(doc! { "started_at": -1 })
            .limit(10)
            .build();
        let mut cursor = self.log_collection.find(doc! {}, find_options).await?;
        
        while let Ok(Some(log)) = cursor.try_next().await {
            logs.push(log);
        }
        
        status.insert("recent_logs".to_string(), serde_json::Value::Array(
            logs.into_iter().map(|log| serde_json::json!({
                "task_id": log.task_id,
                "collection_name": log.collection_name,
                "status": log.status,
                "started_at": format!("{}", log.started_at.timestamp_millis()),
                "completed_at": log.completed_at.map(|dt| format!("{}", dt.timestamp_millis())),
                "videos_collected": log.videos_collected,
                "message": log.message
            })).collect()
        ));

        Ok(status)
    }

    /// 获取任务执行日志
    pub async fn get_task_logs(&self, limit: Option<i32>) -> Result<Vec<TaskExecutionLog>, Box<dyn std::error::Error + Send + Sync>> {
        let limit = limit.unwrap_or(50);
        let find_options = mongodb::options::FindOptions::builder()
            .sort(doc! { "started_at": -1 })
            .limit(limit as i64)
            .build();
        let mut cursor = self.log_collection.find(doc! {}, find_options).await?;
        
        let mut logs = Vec::new();
        while let Ok(Some(log)) = cursor.try_next().await {
            logs.push(log);
        }
        
        Ok(logs)
    }
}