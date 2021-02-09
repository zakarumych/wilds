use {color_eyre::Report, eyre::WrapErr, std::path::PathBuf};

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(untagged)]
pub enum AssetSource {
    FileSystem { path: PathBuf },
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Config {
    pub sources: Vec<AssetSource>,
}

impl Config {
    pub async fn load_default() -> Result<Self, Report> {
        // Load from predefined file path for desktop platforms.
        let path = std::env::var("WILDS_ENGINE_CONFIG_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./cfg.ron"));

        let config = Self::load(path).await?;
        Ok(config)
    }

    #[cfg(not(target = "wasm32"))]
    #[tracing::instrument]
    pub async fn load(path: PathBuf) -> Result<Self, Report> {
        Ok(ron::de::from_reader(std::fs::File::open(&path)?)?)
    }
}
