//! Provides interaction methods with Data Space's OpenSearch API

use anyhow::Result;
use serde::Deserialize;
use serde_json::{Map, Value};
use tracing::info;

/// Feature collection link
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionLink {
    pub rel: String,
    pub href: String,
}

/// Feature collection properties
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureCollectionProperties {
    pub links: Vec<CollectionLink>,
}

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
    properties: FeatureCollectionProperties,
    features: Vec<Product>,
}

/// Sends a query to the OpenSearch API and return the results
pub async fn query(
    endpoint_url: String,
    collection: Option<String>,
    query: Map<String, Value>,
    depaginate: bool,
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
    let mut response: Response = client
        .get(url)
        .query(&parameters)
        .send()
        .await?
        .json()
        .await?;

    let mut result_count = response.features.len();
    info!("Results: {}", result_count);
    let mut products = response.features;
    while let (true, Some(link)) = (
        depaginate,
        response
            .properties
            .links
            .into_iter()
            .find(|link| link.rel == "next"),
    ) {
        response = client.get(link.href).send().await?.json().await?;
        result_count = result_count.saturating_add(response.features.len());
        info!("Results: {}", result_count);
        products.append(&mut response.features);
    }
    Ok(products)
}
