//! Provides a download methods that copies data from Data Space's S3

use anyhow::{Context, Result, anyhow};
use aws_credential_types::{Credentials, provider::SharedCredentialsProvider};
use aws_sdk_s3::{
    Client, Config,
    config::{AppName, BehaviorVersion},
};
use aws_types::region::Region;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::{
    fs::{File, create_dir_all},
    io::copy,
};

/// Storage client instance capable of downloading
#[derive(Clone)]
pub struct StorageClient {
    s3_client: Arc<Client>,
    target: Arc<PathBuf>,
    glob_patterns: Arc<Vec<String>>,
}

impl StorageClient {
    /// Create a storage client
    pub fn init(
        endpoint_url: String,
        access_key_id: String,
        secret_access_key: String,
        target: PathBuf,
        glob_patterns: Vec<String>,
    ) -> Self {
        let credentials = Credentials::from_keys(access_key_id, secret_access_key, None);
        let config = Config::builder()
            .credentials_provider(SharedCredentialsProvider::new(credentials))
            .app_name(AppName::new(env!("CARGO_PKG_NAME")).expect("invalid package name"))
            .behavior_version(BehaviorVersion::latest())
            .endpoint_url(endpoint_url)
            .force_path_style(true)
            .region(Region::new("us-east-1"))
            .build();
        let client = Client::from_conf(config);
        StorageClient {
            s3_client: Arc::new(client),
            target: Arc::new(target),
            glob_patterns: Arc::new(glob_patterns),
        }
    }

    /// Download all parts of a product that match glob patterns onto disk
    pub async fn download_product(&self, key: String) -> Result<Vec<PathBuf>> {
        Ok(vec![])
    }

    /// Download an object onto disk
    async fn download(&self, key: String, relative_to: String) -> Result<PathBuf> {
        // Keys start with a forward slash and the bucket name
        // If not, abort the download
        let bucket = key.split('/').nth(1).ok_or(anyhow!(
            "Product key isn't properly structured (missing bucket): {}",
            key
        ))?;
        let real_key = key.split('/').skip(2).collect::<Vec<_>>().join("/");
        let relative_local_folder = key
            .strip_prefix(&relative_to)
            .ok_or_else(|| anyhow!("Key {} is not relative to {}", key, relative_to))?
            .split('/')
            .rev()
            .skip(1)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("/");
        let local_folder = self.target.join(relative_local_folder);
        create_dir_all(&local_folder)
            .await
            .with_context(|| format!("Couldn't create local folder {}", local_folder.display()))?;
        let mut body = self
            .s3_client
            .get_object()
            .bucket(bucket)
            .key(&real_key)
            .send()
            .await
            .with_context(|| {
                format!(
                    "Failed to download object {:?} from bucket {:?}",
                    key, bucket
                )
            })?
            .body
            .into_async_read();
        let file_name = key
            .split('/')
            .next_back()
            .ok_or(anyhow!("Product key is empty"))?;
        let file_path = local_folder.join(file_name);
        let mut file = File::create(file_path.clone()).await.with_context(|| {
            format!(
                "Failed to create local file {:?} to hold remote object {:?} from bucket {:?}",
                file_path, real_key, bucket
            )
        })?;
        copy(&mut body, &mut file).await.with_context(|| {
            format!(
                "Failed to save the contents of remote object {:?} from bucket {:?} \
                 into local file {:?}",
                real_key, bucket, file_path
            )
        })?;
        Ok(file_path)
    }
}
