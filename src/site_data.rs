use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use mongodb::Database;
use crate::models::{Type, Config};
use futures::stream::TryStreamExt;
use mongodb::bson::doc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationCategory {
    pub category: Type,
    pub sub_categories: Vec<Type>,
}

#[derive(Debug, Clone)]
pub struct SiteData {
    pub navigation_categories: Vec<NavigationCategory>,
    pub all_categories: Vec<Type>,
    pub all_categories_map: HashMap<i32, Type>,
    pub configs: HashMap<String, String>,
    pub last_updated: std::time::Instant,
}

impl SiteData {
    pub fn new() -> Self {
        Self {
            navigation_categories: Vec::new(),
            all_categories: Vec::new(),
            all_categories_map: HashMap::new(),
            configs: HashMap::new(),
            last_updated: std::time::Instant::now(),
        }
    }
}

#[derive(Clone)]
pub struct SiteDataManager {
    data: Arc<RwLock<SiteData>>,
    db: Database,
}

impl SiteDataManager {
    pub fn new(db: Database) -> Self {
        Self {
            data: Arc::new(RwLock::new(SiteData::new())),
            db,
        }
    }

    /// 初始化并加载所有数据
    pub async fn initialize(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("🔧 正在初始化站点数据缓存...");
        
        // 加载分类数据
        self.load_categories().await?;
        
        // 加载配置数据
        self.load_configs().await?;
        
        println!("✅ 站点数据缓存初始化完成");
        Ok(())
    }

    /// 加载分类数据
    async fn load_categories(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let type_collection = self.db.collection::<Type>("types");
        
        // 获取所有分类
        let all_categories: Vec<Type> = type_collection
            .find(None, None)
            .await?
            .try_collect()
            .await
            .unwrap_or_else(|_| vec![]);
        
        // 获取顶级分类用于导航
        let nav_categories: Vec<Type> = all_categories
            .iter()
            .filter(|cat| cat.type_pid == 0 && cat.type_status == 1)
            .cloned()
            .collect();
        
        // 构建导航分类结构
        let mut navigation_categories = Vec::new();
        let mut all_categories_map = HashMap::new();
        
        for category in nav_categories {
            // 获取子分类
            let sub_categories: Vec<Type> = all_categories
                .iter()
                .filter(|cat| cat.type_pid == category.type_id && cat.type_status == 1)
                .cloned()
                .collect();
            
            navigation_categories.push(NavigationCategory {
                category: category.clone(),
                sub_categories,
            });
        }
        
        // 构建分类映射表
        for category in &all_categories {
            all_categories_map.insert(category.type_id, category.clone());
        }
        
        // 更新数据
        let mut data = self.data.write().await;
        data.navigation_categories = navigation_categories;
        data.all_categories = all_categories;
        data.all_categories_map = all_categories_map;
        data.last_updated = std::time::Instant::now();
        
        Ok(())
    }

    /// 加载配置数据
    async fn load_configs(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let config_collection = self.db.collection::<Config>("configs");
        
        let configs: Vec<Config> = config_collection
            .find(None, None)
            .await?
            .try_collect()
            .await
            .unwrap_or_else(|_| vec![]);
        
        let mut config_map = HashMap::new();
        for config in configs {
            config_map.insert(config.config_key, config.config_value);
        }
        
        // 更新数据
        let mut data = self.data.write().await;
        data.configs = config_map;
        data.last_updated = std::time::Instant::now();
        
        Ok(())
    }

    /// 获取导航分类数据
    pub async fn get_navigation_categories(&self) -> Vec<NavigationCategory> {
        let data = self.data.read().await;
        data.navigation_categories.clone()
    }

    /// 获取所有分类
    pub async fn get_all_categories(&self) -> Vec<Type> {
        let data = self.data.read().await;
        data.all_categories.clone()
    }

    /// 根据ID获取分类
    pub async fn get_category_by_id(&self, type_id: i32) -> Option<Type> {
        let data = self.data.read().await;
        data.all_categories_map.get(&type_id).cloned()
    }

    /// 获取配置值
    pub async fn get_config(&self, key: &str) -> Option<String> {
        let data = self.data.read().await;
        data.configs.get(key).cloned()
    }

    /// 获取所有配置
    pub async fn get_all_configs(&self) -> HashMap<String, String> {
        let data = self.data.read().await;
        data.configs.clone()
    }

    /// 刷新数据缓存
    pub async fn refresh(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("🔄 正在刷新站点数据缓存...");
        self.load_categories().await?;
        self.load_configs().await?;
        println!("✅ 站点数据缓存刷新完成");
        Ok(())
    }

    /// 获取缓存统计信息
    pub async fn get_stats(&self) -> HashMap<String, serde_json::Value> {
        let data = self.data.read().await;
        let elapsed = data.last_updated.elapsed();
        
        let mut stats = HashMap::new();
        stats.insert("navigation_categories_count".to_string(), serde_json::json!(data.navigation_categories.len()));
        stats.insert("all_categories_count".to_string(), serde_json::json!(data.all_categories.len()));
        stats.insert("configs_count".to_string(), serde_json::json!(data.configs.len()));
        stats.insert("last_updated_seconds_ago".to_string(), serde_json::json!(elapsed.as_secs()));
        stats.insert("last_updated".to_string(), serde_json::json!(format!("{:?}", data.last_updated)));
        
        stats
    }

    /// 检查缓存是否需要刷新（可选）
    pub async fn needs_refresh(&self, max_age_seconds: u64) -> bool {
        let data = self.data.read().await;
        data.last_updated.elapsed().as_secs() > max_age_seconds
    }
}