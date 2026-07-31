use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct AppPaths {
    app_data_dir: PathBuf,
    database_path: PathBuf,
    images_dir: PathBuf,
}

impl AppPaths {
    pub fn new(app_data_dir: PathBuf) -> std::io::Result<Self> {
        let database_path = app_data_dir.join("library.sqlite3");
        let images_dir = app_data_dir.join("assets").join("images");
        std::fs::create_dir_all(&images_dir)?;

        Ok(Self {
            app_data_dir,
            database_path,
            images_dir,
        })
    }

    pub fn app_data_dir(&self) -> &Path {
        &self.app_data_dir
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn images_dir(&self) -> &Path {
        &self.images_dir
    }

    pub fn resolve_relative(&self, relative_path: &str) -> PathBuf {
        self.app_data_dir.join(relative_path)
    }

    pub fn image_relative_path(id: &str, extension: &str) -> String {
        format!("assets/images/{id}.{extension}")
    }
}
