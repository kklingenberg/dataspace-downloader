//! Provides interaction methods with Data Space's OpenSearch API

use anyhow::Result;
use serde::Deserialize;
use serde_json::{Map, Value};
use tracing::info;

/// A query result obtained from OpenSearch
#[derive(Deserialize)]
pub struct Product {
    pub properties: ProductProperties,
}

/// Feature properties
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductProperties {
    pub product_identifier: String,
}

#[derive(Deserialize)]
struct Response {
    features: Vec<Product>,
}

/// Sends a query to the OpenSearch API and return the results
pub async fn query(
    endpoint_url: String,
    collection: Option<String>,
    query: Map<String, Value>,
    geometry: Option<String>,
) -> Result<Vec<Product>> {
    let url = if let Some(collection_id) = collection {
        format!(
            "{}/{}/search.json",
            endpoint_url.trim_end_matches('/'),
            collection_id
        )
    } else {
        format!("{}/search.json", endpoint_url.trim_end_matches('/'))
    };
    info!("URL: {}", url);

    let mut parameters = query.clone();
    if let Some(geometry_wkt) = geometry {
        // Source:
        // https://documentation.dataspace.copernicus.eu/APIs/OpenSearch.html#geography-and-time-frame
        parameters.insert(
            String::from("geometry"),
            serde_json::Value::String(geometry_wkt),
        );
    }
    info!(
        "Parameters: {}",
        parameters
            .keys()
            .map(String::clone)
            .collect::<Vec<String>>()
            .join(", ")
    );

    let client = reqwest::Client::new();
    let response: Response = client
        .get(url)
        .query(&parameters)
        .send()
        .await?
        .json()
        .await?;

    info!("Results: {}", response.features.len());
    Ok(response.features)
}
