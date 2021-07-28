use super::AppBind;
use anyhow::Error;
use async_trait::async_trait;
use flate2::read::GzDecoder;
use meio::{Context, IdOf, LiteTask, Scheduled, TaskEliminated, TaskError};
use reqwest::Url;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tar::Archive;
use thiserror::Error;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

impl AppBind {
    pub(super) async fn init_assets(&mut self, ctx: &mut Context<Self>) -> Result<(), Error> {
        let path = self
            .options
            .env_var
            .clone()
            .and_then(|env_var| std::env::var(env_var).ok());
        if let Some(path) = path {
            if path.starts_with("http") {
                log::info!("Assets: env-url.");
                let url: Url = path.parse()?;
                ctx.spawn_task(FetchUiPack(url), (), ());
            } else {
                log::info!("Assets: env-path.");
                self.assets = self.read_assets(&path).await?;
                log::warn!("Assets overriden to: {}", path);
            }
        } else if let Some(data) = self.options.embedded.as_ref() {
            log::info!("Assets: embedded.");
            let assets = Assets::parse(data)?;
            self.assets = AssetsMode::Packed(assets);
            log::info!("Embedded assets used.");
        } else if let Some(url) = self.options.url.clone() {
            log::info!("Assets: url.");
            // Load if `env_var` was not set (not overriden)
            ctx.spawn_task(FetchUiPack(url), (), ());
        }
        Ok(())
    }

    async fn read_assets(&mut self, path: &str) -> Result<AssetsMode, Error> {
        let asset_path = Path::new(path).to_path_buf();
        if asset_path.exists() {
            let metadata = tokio::fs::metadata(&asset_path).await?;
            if metadata.is_dir() {
                Ok(AssetsMode::Local(asset_path))
            } else {
                let data = read_file(&asset_path).await?;
                let assets = Assets::parse(&data)?;
                Ok(AssetsMode::Packed(assets))
            }
        } else {
            Err(Error::msg(format!("Can't load assets from {}", path)))
        }
    }
}

pub async fn read_file(path: &Path) -> Result<Vec<u8>, Error> {
    let mut file = File::open(path).await?;
    let mut content = Vec::new();
    file.read_to_end(&mut content).await?;
    Ok(content)
}

pub enum AssetsMode {
    Loading,
    Local(PathBuf),
    Packed(Assets),
    //Proxy(Uri),
    Failed(String),
}

#[derive(Debug, Error)]
enum Reason {
    #[error("wrong format")]
    WrongFormat,
}

#[derive(Clone, Debug)]
pub struct Assets {
    files: Arc<HashMap<String, Vec<u8>>>,
}

impl Assets {
    /// Expected gzipped tar file contents.
    pub fn parse(assets: &[u8]) -> Result<Assets, Error> {
        let tar = GzDecoder::new(assets);
        let mut archive = Archive::new(tar);
        let mut files = HashMap::new();
        for entry in archive.entries()? {
            let mut entry = entry?;
            let mut data = Vec::new();
            entry.read_to_end(&mut data)?;
            if !data.is_empty() {
                let name = entry
                    .path()?
                    .to_str()
                    .ok_or(Reason::WrongFormat)?
                    .to_owned();
                #[cfg(debug_assertions)]
                log::trace!("Register asset file: {}", name);
                files.insert(name, data);
            }
        }
        Ok(Self {
            files: Arc::new(files),
        })
    }

    pub fn get(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(Vec::as_ref)
    }
}

pub struct FetchUiPack(Url);

#[async_trait]
impl LiteTask for FetchUiPack {
    type Output = Assets;

    async fn interruptable_routine(mut self) -> Result<Self::Output, Error> {
        log::info!("Fetching UI assets...");
        let bytes = reqwest::get(self.0)
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        let assets = Assets::parse(&bytes)?;
        Ok(assets)
    }
}

#[async_trait]
impl TaskEliminated<FetchUiPack, ()> for AppBind {
    async fn handle(
        &mut self,
        _id: IdOf<FetchUiPack>,
        _tag: (),
        result: Result<Assets, TaskError>,
        ctx: &mut Context<Self>,
    ) -> Result<(), Error> {
        match result {
            Ok(assets) => {
                self.assets = AssetsMode::Packed(assets);
                log::info!("Assets pack attached.");
                Ok(())
            }
            Err(err) => {
                self.assets = AssetsMode::Failed(err.to_string());
                // TODO: Use `meio::after!(5 seconds)`.
                ctx.address()
                    .schedule(ReInitAssets, Instant::now() + Duration::from_secs(5))?;
                // TODO: Schedule refetching...
                log::error!("Can't load UI pack: {}", err);
                Err(err.into())
            }
        }
    }
}

struct ReInitAssets;

#[async_trait]
impl Scheduled<ReInitAssets> for AppBind {
    async fn handle(
        &mut self,
        _: Instant,
        _: ReInitAssets,
        ctx: &mut Context<Self>,
    ) -> Result<(), Error> {
        self.init_assets(ctx).await?;
        Ok(())
    }
}
