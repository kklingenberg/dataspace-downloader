mod catalog;
mod storage;

use anyhow::{Context, Result, anyhow};
use clap::{Parser, value_parser};
use futures::StreamExt;
use geo_types::Geometry;
use geojson::GeoJson;
use glob::Pattern;
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use wkt::to_wkt::ToWkt;

/// Query Copernicus Dataspace and download their assets from S3
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// S3 endpoint URL, which defaults to https://eodata.dataspace.copernicus.eu/
    #[arg(long, env)]
    s3_endpoint_url: Option<String>,

    /// S3 access key id
    #[arg(long, env)]
    s3_access_key_id: Option<String>,

    /// S3 secret_access key
    #[arg(long, env)]
    s3_secret_access_key: Option<String>,

    /// Keys file, optional; must be given if keys are not given inline
    #[arg(short, long, env)]
    keys_file: Option<PathBuf>,

    /// Configuration file (query parameters)
    #[arg(short, long, env)]
    config: Option<PathBuf>,

    /// File with geometry of interest (GeoJSON format)
    #[arg(short, long, env)]
    geometry: Option<PathBuf>,

    /// The target directory where files will be downloaded; defaults
    /// to current directory
    #[arg(short, long, env)]
    output: Option<PathBuf>,

    /// Number of products to download in parallel
    #[arg(short, long, env, default_value_t = 5, value_parser = value_parser!(u16).range(1..))]
    parallelism: u16,

    /// Skip downloading, only list results
    #[arg(long, action)]
    no_download: bool,

    /// Logging verbosity level
    #[arg(long, env, default_value_t = tracing::Level::INFO)]
    log_level: tracing::Level,
}

/// Source: https://documentation.dataspace.copernicus.eu/APIs/OpenSearch.html#general-rules
fn default_opensearch_endpoint() -> String {
    String::from("https://catalogue.dataspace.copernicus.eu/resto/api/collections/")
}

/// Source:
/// https://documentation.dataspace.copernicus.eu/APIs/S3.html#object-storage-endpoints
fn default_s3_endpoint() -> String {
    String::from("https://eodata.dataspace.copernicus.eu/")
}

/// S3 keys configuration block
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KeysConfiguration {
    #[serde(default = "default_s3_endpoint")]
    endpoint_url: String,
    access_key_id: String,
    secret_access_key: String,
}

impl Default for KeysConfiguration {
    fn default() -> Self {
        KeysConfiguration {
            endpoint_url: default_s3_endpoint(),
            access_key_id: String::from("not-set"),
            secret_access_key: String::from("not-set"),
        }
    }
}

/// Query and download configuration
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Configuration {
    #[serde(default = "default_opensearch_endpoint")]
    endpoint_url: String,

    #[serde(default)]
    collection: Option<String>,

    #[serde(default)]
    query: serde_json::Map<String, serde_json::Value>,

    #[serde(default)]
    depaginate: bool,

    #[serde(default)]
    glob_patterns: Vec<String>,
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            endpoint_url: default_opensearch_endpoint(),
            collection: None,
            query: serde_json::Map::<String, serde_json::Value>::default(),
            depaginate: false,
            glob_patterns: Vec::default(),
        }
    }
}

/// Extract a WKT for points, polygons or multipolygons from given geometry
fn extract_wkt(geometry: Geometry) -> Result<String> {
    match geometry {
        Geometry::GeometryCollection(gc) if gc.len() == 1 => extract_wkt(gc[0].clone()),
        Geometry::Point(p) => Ok(p.wkt_string()),
        Geometry::Polygon(p) => Ok(p.wkt_string()),
        Geometry::MultiPolygon(mp) => Ok(mp.wkt_string()),
        _ => Err(anyhow!("Geometry is not a polygon or multipolygon")),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_max_level(cli.log_level)
        .with_target(false)
        .without_time()
        .init();

    let keys: KeysConfiguration = cli.keys_file.map_or_else(
        || Ok(KeysConfiguration::default()),
        |path| {
            let file = File::open(&path)
                .with_context(|| format!("Couldn't open keys file {}", path.display()))?;
            let reader = BufReader::new(file);
            serde_json::from_reader(reader).context("Keys file is not properly JSON-encoded")
        },
    )?;

    let config: Configuration = cli.config.map_or_else(
        || Ok(Configuration::default()),
        |path| {
            let file = File::open(&path)
                .with_context(|| format!("Couldn't open configuration file {}", path.display()))?;
            let reader = BufReader::new(file);
            serde_json::from_reader(reader)
                .context("Configuration file is not properly JSON-encoded")
        },
    )?;

    let geometry: Option<String> = cli.geometry.map_or(Ok(None), |path| {
        let file = File::open(&path)
            .with_context(|| format!("Couldn't open geometry file {}", path.display()))?;
        let reader = BufReader::new(file);
        let geojson: GeoJson = serde_json::from_reader(reader)
            .context("Geometry file is not properly GeoJSON-encoded")?;
        Geometry::try_from(geojson)
            .context("Couldn't convert GeoJSON into simple geometry")
            .and_then(extract_wkt)
            .map(Some)
    })?;

    let products: Vec<catalog::Product> = catalog::query(
        config.endpoint_url,
        config.collection,
        config.query,
        config.depaginate,
        geometry,
    )
    .await?;
    if cli.no_download {
        for product in products {
            println!("{}", product.properties.product_identifier);
        }
        return Ok(());
    }

    let storage_client = storage::StorageClient::init(
        cli.s3_endpoint_url.unwrap_or(keys.endpoint_url),
        cli.s3_access_key_id.unwrap_or(keys.access_key_id),
        cli.s3_secret_access_key.unwrap_or(keys.secret_access_key),
        cli.output.unwrap_or(PathBuf::from(".")),
        config
            .glob_patterns
            .iter()
            .map(|p| Pattern::new(p).with_context(|| format!("Couldn't build glob pattern: {}", p)))
            .collect::<Result<Vec<_>>>()?,
    );
    futures::stream::iter(products.into_iter().map(|product| {
        let client = storage_client.clone();
        tokio::spawn(async move {
            client
                .download_product(&product.properties.product_identifier)
                .await
        })
    }))
    .buffer_unordered(5)
    .map(|result| match result {
        Ok(Ok(paths)) => paths,
        anything_else => {
            println!("Couldn't download collection item: {:?}", anything_else);
            Vec::new()
        }
    })
    .collect::<Vec<_>>()
    .await
    .iter()
    .flatten()
    .for_each(|path| println!("Downloaded {}", path.display()));
    Ok(())
}
