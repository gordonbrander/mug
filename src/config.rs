use std::path::PathBuf;

#[allow(dead_code)]
pub struct Config {
    pub content_dir: PathBuf,
    pub output_dir: PathBuf,
    pub templates_dir: PathBuf,
    pub static_dir: PathBuf,
    pub data_dir: PathBuf,
    pub generators_dir: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            content_dir: PathBuf::from("content"),
            output_dir: PathBuf::from("dist"),
            templates_dir: PathBuf::from("templates"),
            static_dir: PathBuf::from("static"),
            data_dir: PathBuf::from("data"),
            generators_dir: PathBuf::from("generators"),
        }
    }
}
