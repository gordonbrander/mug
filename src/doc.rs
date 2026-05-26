use chrono::{DateTime, Utc};
use serde_yaml_ng::Mapping;
use std::path::PathBuf;

#[allow(dead_code)]
pub struct Doc {
    pub id_path: PathBuf,
    pub output_path: PathBuf,
    pub template: Option<String>,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub date: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub data: Mapping,
}

impl Doc {
    pub fn from_body(id_path: PathBuf, body: String) -> Doc {
        let output_path = id_path.with_extension("html");
        let now = Utc::now();
        Doc {
            id_path,
            output_path,
            template: None,
            title: String::new(),
            content: body,
            tags: Vec::new(),
            date: now,
            updated: now,
            data: Mapping::new(),
        }
    }
}
