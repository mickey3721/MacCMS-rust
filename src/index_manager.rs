use mongodb::{Database, IndexModel, options::IndexOptions};
use mongodb::bson::{doc, Document};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use futures::TryStreamExt;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CollectionIndexInfo {
    pub collection_name: String,
    pub indexes: Vec<SingleIndexInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SingleIndexInfo {
    pub name: String,
    pub keys: HashMap<String, i32>,
    pub unique: bool,
    pub sparse: bool,
    pub background: bool,
    pub version: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexInfo {
    pub collection: String,
    pub keys: HashMap<String, i32>,
    pub name: String,
    pub unique: Option<bool>,
    pub sparse: Option<bool>,
    pub background: Option<bool>,
}

pub struct IndexManager {
    db: Database,
}

impl IndexManager {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// 获取所有需要创建的索引配置
    fn get_index_configs() -> Vec<IndexInfo> {
        vec![
            // vods 集合索引
            IndexInfo {
                collection: "vods".to_string(),
                keys: {
                    let mut keys = HashMap::new();
                    keys.insert("vod_name".to_string(), 1);
                    keys.insert("vod_year".to_string(), 1);
                    keys
                },
                name: "vod_name_1_vod_year_1".to_string(),
                unique: Some(true),
                sparse: Some(true),
                background: Some(true),
            },
            IndexInfo {
                collection: "vods".to_string(),
                keys: {
                    let mut keys = HashMap::new();
                    keys.insert("type_id".to_string(), 1);
                    keys
                },
                name: "type_id_1".to_string(),
                unique: None,
                sparse: None,
                background: Some(true),
            },
            IndexInfo {
                collection: "vods".to_string(),
                keys: {
                    let mut keys = HashMap::new();
                    keys.insert("vod_pubdate".to_string(), -1);
                    keys
                },
                name: "vod_pubdate_-1".to_string(),
                unique: None,
                sparse: None,
                background: Some(true),
            },
            IndexInfo {
                collection: "vods".to_string(),
                keys: {
                    let mut keys = HashMap::new();
                    keys.insert("vod_year".to_string(), 1);
                    keys
                },
                name: "vod_year_1".to_string(),
                unique: None,
                sparse: Some(true),
                background: Some(true),
            },
            IndexInfo {
                collection: "vods".to_string(),
                keys: {
                    let mut keys = HashMap::new();
                    keys.insert("vod_area".to_string(), 1);
                    keys
                },
                name: "vod_area_1".to_string(),
                unique: None,
                sparse: Some(true),
                background: Some(true),
            },
            IndexInfo {
                collection: "vods".to_string(),
                keys: {
                    let mut keys = HashMap::new();
                    keys.insert("vod_status".to_string(), 1);
                    keys.insert("vod_pubdate".to_string(), -1);
                    keys
                },
                name: "vod_status_1_vod_pubdate_-1".to_string(),
                unique: None,
                sparse: None,
                background: Some(true),
            },
            IndexInfo {
                collection: "vods".to_string(),
                keys: {
                    let mut keys = HashMap::new();
                    keys.insert("type_id".to_string(), 1);
                    keys.insert("vod_pubdate".to_string(), -1);
                    keys
                },
                name: "type_id_1_vod_pubdate_-1".to_string(),
                unique: None,
                sparse: None,
                background: Some(true),
            },
            
            // types 集合索引
            IndexInfo {
                collection: "types".to_string(),
                keys: {
                    let mut keys = HashMap::new();
                    keys.insert("type_id".to_string(), 1);
                    keys
                },
                name: "type_id_1".to_string(),
                unique: Some(true),
                sparse: None,
                background: Some(true),
            },
            IndexInfo {
                collection: "types".to_string(),
                keys: {
                    let mut keys = HashMap::new();
                    keys.insert("type_pid".to_string(), 1);
                    keys.insert("type_sort".to_string(), 1);
                    keys
                },
                name: "type_pid_1_type_sort_1".to_string(),
                unique: None,
                sparse: None,
                background: Some(true),
            },
            
            // bindings 集合索引
            IndexInfo {
                collection: "bindings".to_string(),
                keys: {
                    let mut keys = HashMap::new();
                    keys.insert("source_flag".to_string(), 1);
                    keys.insert("external_id".to_string(), 1);
                    keys
                },
                name: "source_flag_1_external_id_1".to_string(),
                unique: Some(true),
                sparse: Some(true), // 使用稀疏索引避免空值问题
                background: Some(true),
            },
            IndexInfo {
                collection: "bindings".to_string(),
                keys: {
                    let mut keys = HashMap::new();
                    keys.insert("local_type_id".to_string(), 1);
                    keys
                },
                name: "local_type_id_1".to_string(),
                unique: None,
                sparse: None,
                background: Some(true),
            },
            
            // collections 集合索引
            IndexInfo {
                collection: "collections".to_string(),
                keys: {
                    let mut keys = HashMap::new();
                    keys.insert("collect_status".to_string(), 1);
                    keys.insert("collect_type".to_string(), 1);
                    keys
                },
                name: "collect_status_1_collect_type_1".to_string(),
                unique: None,
                sparse: None,
                background: Some(true),
            },
            IndexInfo {
                collection: "collections".to_string(),
                keys: {
                    let mut keys = HashMap::new();
                    keys.insert("created_at".to_string(), -1);
                    keys
                },
                name: "created_at_-1".to_string(),
                unique: None,
                sparse: None,
                background: Some(true),
            },
            
            // configs 集合索引
            IndexInfo {
                collection: "configs".to_string(),
                keys: {
                    let mut keys = HashMap::new();
                    keys.insert("config_key".to_string(), 1);
                    keys
                },
                name: "config_key_1".to_string(),
                unique: Some(true),
                sparse: None,
                background: Some(true),
            },
            IndexInfo {
                collection: "configs".to_string(),
                keys: {
                    let mut keys = HashMap::new();
                    keys.insert("config_group".to_string(), 1);
                    keys.insert("config_sort".to_string(), 1);
                    keys
                },
                name: "config_group_1_config_sort_1".to_string(),
                unique: None,
                sparse: None,
                background: Some(true),
            },
        ]
    }

    /// 检查索引是否已存在
    async fn index_exists(&self, collection_name: &str, index_name: &str) -> Result<bool, mongodb::error::Error> {
        let collection = self.db.collection::<mongodb::bson::Document>(collection_name);
        
        // 使用 list_indexes 获取索引列表，然后检查指定名称是否存在
        match collection.list_indexes(None).await {
            Ok(mut cursor) => {
                let mut found = false;
                while let Ok(Some(index_model)) = cursor.try_next().await {
                    if let Some(options) = &index_model.options {
                        if let Some(name) = &options.name {
                            if name == index_name {
                                found = true;
                                break;
                            }
                        }
                    }
                }
                Ok(found)
            }
            Err(e) => {
                eprintln!("❌ 获取索引列表失败: {}.{}: {}", collection_name, index_name, e);
                Err(e)
            }
        }
    }

    /// 创建单个索引（先检查是否存在）
    async fn create_index(&self, collection_name: &str, index_info: &IndexInfo) -> Result<(), mongodb::error::Error> {
        // 先检查索引是否已存在
        match self.index_exists(collection_name, &index_info.name).await {
            Ok(true) => {
                println!("⚪ 索引已存在，跳过: {} on {}", index_info.name, collection_name);
                return Ok(());
            }
            Ok(false) => {
                // 索引不存在，继续创建
            }
            Err(e) => {
                eprintln!("❌ 检查索引存在性失败: {} on {}: {}", index_info.name, collection_name, e);
                // 继续尝试创建索引
            }
        }

        let collection = self.db.collection::<mongodb::bson::Document>(collection_name);
        
        // 构建索引选项
        let mut options = IndexOptions::default();
        if let Some(unique) = index_info.unique {
            options.unique = Some(unique);
        }
        if let Some(sparse) = index_info.sparse {
            options.sparse = Some(sparse);
        }
        if let Some(background) = index_info.background {
            options.background = Some(background);
        }
        
        // 构建键文档
        let mut keys_doc = Document::new();
        for (key, value) in &index_info.keys {
            keys_doc.insert(key, *value);
        }
        
        // 构建索引模型
        let index_model = IndexModel::builder()
            .keys(keys_doc)
            .options(options)
            .build();

        // 创建索引
        match collection.create_index(index_model, None).await {
            Ok(_) => {
                println!("✅ 成功创建索引: {} on {}", index_info.name, collection_name);
                Ok(())
            }
            Err(e) => {
                if e.to_string().contains("already exists") {
                    println!("⚪ 索引已存在: {} on {}", index_info.name, collection_name);
                    Ok(())
                } else {
                    eprintln!("❌ 创建索引失败: {} on {}: {}", index_info.name, collection_name, e);
                    Err(e)
                }
            }
        }
    }

    /// 创建所有需要的索引
    pub async fn create_all_indexes(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🚀 开始创建数据库索引...");
        
        let index_configs = Self::get_index_configs();
        let mut success_count = 0;
        let mut error_count = 0;

        for index_info in index_configs {
            match self.create_index(&index_info.collection, &index_info).await {
                Ok(_) => success_count += 1,
                Err(_) => error_count += 1,
            }
        }

        println!("📊 索引创建完成: 成功 {}, 失败 {}", success_count, error_count);
        
        if error_count > 0 {
            return Err("部分索引创建失败".into());
        }

        Ok(())
    }

    /// 验证索引是否存在
    pub async fn verify_indexes(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔍 验证数据库索引...");
        
        let index_configs = Self::get_index_configs();
        let mut missing_indexes = Vec::new();

        for index_info in index_configs {
            match self.index_exists(&index_info.collection, &index_info.name).await {
                Ok(true) => {
                    // 索引存在，继续检查下一个
                }
                Ok(false) => {
                    missing_indexes.push(format!("{}.{}", index_info.collection, index_info.name));
                }
                Err(e) => {
                    eprintln!("❌ 检查索引失败: {}.{}: {}", index_info.collection, index_info.name, e);
                    missing_indexes.push(format!("{}.{}", index_info.collection, index_info.name));
                }
            }
        }

        if missing_indexes.is_empty() {
            println!("✅ 所有索引验证通过");
            Ok(())
        } else {
            eprintln!("❌ 缺失的索引: {:?}", missing_indexes);
            Err(format!("缺失 {} 个索引", missing_indexes.len()).into())
        }
    }

    /// 显示当前索引状态
    pub async fn show_index_status(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("📋 数据库索引状态:");
        
        let collections = vec!["vods", "types", "bindings", "collections", "configs"];
        
        for collection_name in collections {
            println!("\n📁 {}:", collection_name);
            let collection = self.db.collection::<mongodb::bson::Document>(collection_name);
            
            match collection.list_indexes(None).await {
                Ok(mut cursor) => {
                    let mut index_names = Vec::new();
                    while let Ok(Some(index_model)) = cursor.try_next().await {
                        if let Some(options) = &index_model.options {
                            if let Some(name) = &options.name {
                                index_names.push(name.clone());
                            }
                        }
                    }
                    
                    if index_names.is_empty() {
                        println!("  无索引");
                    } else {
                        for (i, name) in index_names.iter().enumerate() {
                            if name != "_id_" { // 跳过默认的_id索引
                                println!("  {}. {}", i + 1, name);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("  ❌ 获取索引失败: {}", e);
                }
            }
        }
        
        Ok(())
    }

    /// 获取所有集合的索引信息
    pub async fn get_all_indexes(&self) -> Result<Vec<CollectionIndexInfo>, Box<dyn std::error::Error>> {
        let collections = vec!["vods", "types", "bindings", "collections", "configs"];
        let mut result = Vec::new();
        
        for collection_name in collections {
            let collection = self.db.collection::<mongodb::bson::Document>(collection_name);
            
            match collection.list_indexes(None).await {
                Ok(mut cursor) => {
                    let mut indexes = Vec::new();
                    
                    while let Ok(Some(index_model)) = cursor.try_next().await {
                        let options = index_model.options.as_ref();
                        
                        // 跳过默认的_id索引
                        if let Some(name) = options.and_then(|opts| opts.name.as_ref()) {
                            if name == "_id_" {
                                continue;
                            }
                        }
                        
                        // 解析索引键
                        let mut keys = HashMap::new();
                        for (key, value) in index_model.keys.iter() {
                            if let Some(ival) = value.as_i32() {
                                keys.insert(key.to_string(), ival);
                            } else if let Some(bval) = value.as_i64() {
                                keys.insert(key.to_string(), bval as i32);
                            }
                        }
                        
                        let index_info = SingleIndexInfo {
                            name: options.and_then(|opts| opts.name.as_ref())
                                .unwrap_or(&"unknown".to_string()).to_string(),
                            keys,
                            unique: options.and_then(|opts| opts.unique).unwrap_or(false),
                            sparse: options.and_then(|opts| opts.sparse).unwrap_or(false),
                            background: options.and_then(|opts| opts.background).unwrap_or(false),
                            version: None, // 暂时设置为None，因为IndexVersion类型转换复杂
                        };
                        
                        indexes.push(index_info);
                    }
                    
                    let collection_info = CollectionIndexInfo {
                        collection_name: collection_name.to_string(),
                        indexes,
                    };
                    
                    result.push(collection_info);
                }
                Err(e) => {
                    eprintln!("❌ 获取集合 {} 索引失败: {}", collection_name, e);
                }
            }
        }
        
        Ok(result)
    }
}