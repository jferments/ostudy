use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct AppPaths {
    app_data_dir: PathBuf,
    database_path: PathBuf,
    images_dir: PathBuf,
    asset_staging_dir: PathBuf,
    asset_quarantine_dir: PathBuf,
    documents_dir: PathBuf,
}

impl AppPaths {
    pub fn new(app_data_dir: PathBuf) -> std::io::Result<Self> {
        let database_path = app_data_dir.join("library.sqlite3");
        let assets_dir = app_data_dir.join("assets");
        let images_dir = assets_dir.join("images");
        let asset_staging_dir = assets_dir.join("staging");
        let asset_quarantine_dir = assets_dir.join("quarantine");
        let documents_dir = app_data_dir.join("documents");
        let document_staging_dir = documents_dir.join("staging");
        let transfer_imports_dir = app_data_dir.join("transfers").join("imports");
        let transfer_exports_dir = app_data_dir.join("transfers").join("exports");
        std::fs::create_dir_all(&images_dir)?;
        std::fs::create_dir_all(&asset_staging_dir)?;
        std::fs::create_dir_all(&asset_quarantine_dir)?;
        std::fs::create_dir_all(&documents_dir)?;
        std::fs::create_dir_all(&document_staging_dir)?;
        std::fs::create_dir_all(&transfer_imports_dir)?;
        std::fs::create_dir_all(&transfer_exports_dir)?;

        Ok(Self {
            app_data_dir,
            database_path,
            images_dir,
            asset_staging_dir,
            asset_quarantine_dir,
            documents_dir,
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

    pub fn asset_staging_dir(&self) -> &Path {
        &self.asset_staging_dir
    }

    pub fn asset_quarantine_dir(&self) -> &Path {
        &self.asset_quarantine_dir
    }

    pub fn document_directory(&self, document_id: &str) -> PathBuf {
        self.documents_dir.join(document_id)
    }

    pub fn document_pages_dir(&self, document_id: &str) -> PathBuf {
        self.document_directory(document_id).join("pages")
    }

    pub fn document_assets_dir(&self, document_id: &str) -> PathBuf {
        self.document_directory(document_id).join("assets")
    }

    pub fn resolve_relative(&self, relative_path: &str) -> PathBuf {
        self.app_data_dir.join(relative_path)
    }

    pub fn image_relative_path(id: &str, extension: &str) -> String {
        format!("assets/images/{id}.{extension}")
    }

    pub fn staged_asset_relative_path(draft_id: &str, asset_id: &str, extension: &str) -> String {
        format!("assets/staging/{draft_id}-{asset_id}.{extension}.stage")
    }

    pub fn quarantined_asset_relative_path(asset_id: &str, suffix: &str) -> String {
        format!("assets/quarantine/{asset_id}-{suffix}.quarantine")
    }

    /// Compatibility helper for existing PDF data and tests.
    pub fn document_source_relative_path(document_id: &str) -> String {
        Self::document_source_relative_path_with_extension(document_id, "pdf")
    }

    pub fn document_source_relative_path_with_extension(
        document_id: &str,
        extension: &str,
    ) -> String {
        format!(
            "documents/{document_id}/source.{}",
            safe_extension(extension)
        )
    }

    pub fn document_versioned_asset_relative_path(
        document_id: &str,
        asset_ordinal: usize,
        content_fingerprint: u64,
        extension: &str,
    ) -> String {
        format!(
            "documents/{document_id}/assets/asset-{asset_ordinal:06}{content_fingerprint:020}.{}",
            safe_extension(extension)
        )
    }

    pub fn document_page_relative_path(document_id: &str, page_number: i64) -> String {
        format!("documents/{document_id}/pages/page-{page_number:06}.png")
    }

    pub fn staged_document_relative_path_with_extension(
        document_id: &str,
        extension: &str,
    ) -> String {
        format!(
            "documents/staging/{document_id}.{}.stage",
            safe_extension(extension)
        )
    }

    pub fn transfer_imports_dir(&self) -> PathBuf {
        self.app_data_dir.join("transfers").join("imports")
    }

    pub fn transfer_exports_dir(&self) -> PathBuf {
        self.app_data_dir.join("transfers").join("exports")
    }

    pub fn staged_import_partial_relative_path(id: &str) -> String {
        format!("transfers/imports/{id}.partial")
    }

    pub fn staged_import_relative_path(id: &str, extension: &str) -> String {
        format!("transfers/imports/{id}.{}", safe_extension(extension))
    }

    pub fn prepared_export_relative_path(id: &str, extension: &str) -> String {
        format!("transfers/exports/{id}.{}", safe_extension(extension))
    }
}

fn safe_extension(extension: &str) -> &str {
    if !extension.is_empty()
        && extension.len() <= 16
        && extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        extension
    } else {
        "bin"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_document_paths_preserve_safe_extensions() {
        assert_eq!(
            AppPaths::document_source_relative_path_with_extension("abc", "docx"),
            "documents/abc/source.docx"
        );
        assert_eq!(
            AppPaths::document_versioned_asset_relative_path("abc", 3, 42, "png"),
            "documents/abc/assets/asset-00000300000000000000000042.png"
        );
        assert_eq!(
            AppPaths::document_source_relative_path_with_extension("abc", "../exe"),
            "documents/abc/source.bin"
        );
    }
}
