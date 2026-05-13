use std::time::Duration;

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

const SEARCH_URL: &str = "https://world.openfoodfacts.org/cgi/search.pl";
const PRODUCT_URL: &str = "https://world.openfoodfacts.org/api/v2/product";

/// Nutrition data for a food item, either per 100g or per serving.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nutrition {
    pub kcal: f64,
    pub fat_g: f64,
    pub protein_g: f64,
    pub carbs_g: f64,
}

/// A single search result from Open Food Facts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoodSearchResult {
    pub product_name: String,
    pub barcode: String,
    pub quantity: Option<String>,
    pub per_100g: Nutrition,
    pub serving: Option<ServingInfo>,
}

/// Serving size information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServingInfo {
    pub size: String,
    pub nutrition: Nutrition,
}

/// A full product lookup result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductInfo {
    pub product_name: String,
    pub barcode: String,
    pub per_100g: Nutrition,
    pub serving: Option<ServingInfo>,
}

// ---------------------------------------------------------------------------
// Raw API response shapes (only fields we care about)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SearchResponse {
    products: Vec<RawProduct>,
}

#[derive(Deserialize)]
struct ProductResponse {
    #[allow(dead_code)]
    status: u32,
    product: Option<RawProduct>,
    #[allow(dead_code)]
    status_verbose: Option<String>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
struct RawProduct {
    #[serde(default)]
    product_name: Option<String>,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    quantity: Option<String>,
    #[serde(default)]
    serving_size: Option<String>,
    #[serde(default)]
    nutriments: RawNutriments,
    /// Nested product field used by some API response shapes.
    #[serde(default)]
    product: Option<Box<RawProduct>>,
}

#[derive(Deserialize, Clone, Default)]
#[serde(rename_all = "snake_case")]
struct RawNutriments {
    #[serde(default, alias = "energy-kcal_100g")]
    energy_kcal_100g: Option<f64>,
    #[serde(default)]
    fat_100g: Option<f64>,
    #[serde(default)]
    proteins_100g: Option<f64>,
    #[serde(default)]
    carbohydrates_100g: Option<f64>,

    #[serde(default, alias = "energy-kcal_serving")]
    energy_kcal_serving: Option<f64>,
    #[serde(default)]
    fat_serving: Option<f64>,
    #[serde(default)]
    proteins_serving: Option<f64>,
    #[serde(default)]
    carbohydrates_serving: Option<f64>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Search Open Food Facts for products matching `query`.
pub fn search(query: &str) -> AppResult<Vec<FoodSearchResult>> {
    let url = format!(
        "{}?search_terms={}&json=1&page_size=10",
        SEARCH_URL,
        urlencode(query)
    );

    let body = http_get_with_retry(&url, 3)?;

    let response: SearchResponse = serde_json::from_str(&body)
        .map_err(|e| AppError::FoodApi(format!("Search JSON parse failed: {e}")))?;

    let mut results = Vec::new();
    for raw in response.products {
        match raw_to_search_result(raw) {
            Ok(r) => results.push(r),
            Err(e) => tracing::warn!("Skipping search result: {e}"),
        }
    }

    if results.is_empty() {
        return Err(AppError::FoodApi(format!(
            "No products found for '{query}'"
        )));
    }

    Ok(results)
}

/// Look up a single product by its barcode.
pub fn lookup_barcode(barcode: &str) -> AppResult<ProductInfo> {
    let url = format!("{}/{}.json", PRODUCT_URL, barcode);

    let body = http_get_with_retry(&url, 2)?;

    let response: ProductResponse = serde_json::from_str(&body)
        .map_err(|e| AppError::FoodApi(format!("Product JSON parse failed: {e}")))?;

    if response.status != 1 {
        return Err(AppError::FoodApi(format!(
            "Product not found for barcode '{barcode}'"
        )));
    }

    let raw = response
        .product
        .ok_or_else(|| AppError::FoodApi(format!("Empty product data for '{barcode}'")))?;

    raw_to_product_info(raw)
}

/// Given per-100g nutrition and a weight in grams, compute the actual nutrition.
pub fn compute_nutrition(per_100g: &Nutrition, grams: f64) -> Nutrition {
    let factor = grams / 100.0;
    Nutrition {
        kcal: per_100g.kcal * factor,
        fat_g: per_100g.fat_g * factor,
        protein_g: per_100g.protein_g * factor,
        carbs_g: per_100g.carbs_g * factor,
    }
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/// Parsed fields common to search results and product lookups.
struct ParsedRaw {
    product_name: String,
    barcode: String,
    quantity: Option<String>,
    per_100g: Nutrition,
    serving: Option<ServingInfo>,
}

fn parse_raw_product(raw: RawProduct) -> AppResult<ParsedRaw> {
    // Search results may have the product nested inside a top-level "product" field.
    let actual = match raw.product {
        Some(p) => *p,
        None => raw,
    };

    let product_name = actual
        .product_name
        .ok_or_else(|| AppError::FoodApi("Missing product name".into()))?;
    let barcode = actual
        .code
        .ok_or_else(|| AppError::FoodApi("Missing barcode".into()))?;

    let per_100g = parse_per_100g(&actual.nutriments);
    let serving = parse_serving(&actual.nutriments, &actual.serving_size);

    Ok(ParsedRaw {
        product_name,
        barcode,
        quantity: actual.quantity,
        per_100g,
        serving,
    })
}

fn raw_to_search_result(raw: RawProduct) -> AppResult<FoodSearchResult> {
    let p = parse_raw_product(raw)?;
    Ok(FoodSearchResult {
        product_name: p.product_name,
        barcode: p.barcode,
        quantity: p.quantity,
        per_100g: p.per_100g,
        serving: p.serving,
    })
}

fn raw_to_product_info(raw: RawProduct) -> AppResult<ProductInfo> {
    let p = parse_raw_product(raw)?;
    Ok(ProductInfo {
        product_name: p.product_name,
        barcode: p.barcode,
        per_100g: p.per_100g,
        serving: p.serving,
    })
}

fn parse_per_100g(n: &RawNutriments) -> Nutrition {
    Nutrition {
        kcal: n.energy_kcal_100g.unwrap_or(0.0),
        fat_g: n.fat_100g.unwrap_or(0.0),
        protein_g: n.proteins_100g.unwrap_or(0.0),
        carbs_g: n.carbohydrates_100g.unwrap_or(0.0),
    }
}

fn parse_serving(n: &RawNutriments, serving_size: &Option<String>) -> Option<ServingInfo> {
    let size = serving_size.as_ref()?.trim().to_string();
    if size.is_empty() {
        return None;
    }
    // Only return serving info if at least kcal_serving is present.
    n.energy_kcal_serving?;

    Some(ServingInfo {
        size,
        nutrition: Nutrition {
            kcal: n.energy_kcal_serving.unwrap_or(0.0),
            fat_g: n.fat_serving.unwrap_or(0.0),
            protein_g: n.proteins_serving.unwrap_or(0.0),
            carbs_g: n.carbohydrates_serving.unwrap_or(0.0),
        },
    })
}

fn urlencode(s: &str) -> String {
    // Percent-encode per RFC 3986: unreserved chars pass through,
    // space becomes '+', everything else is percent-encoded.
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            b' ' => "+".to_string(),
            _ => format!("%{:02X}", b),
        })
        .collect()
}

/// HTTP GET with retries and exponential backoff for transient failures (503, 429, 502).
fn http_get_with_retry(url: &str, max_retries: u32) -> AppResult<String> {
    let mut last_error = None;

    for attempt in 0..=max_retries {
        if attempt > 0 {
            let delay = Duration::from_millis(500 * 2u64.pow(attempt - 1));
            std::thread::sleep(delay);
        }

        let result = ureq::get(url)
            .set("User-Agent", "GlowDiary/0.1 (food-diary-mcp)")
            .call();

        match result {
            Ok(response) => {
                let status = response.status();
                if status == 200 {
                    return response
                        .into_string()
                        .map_err(|e| AppError::FoodApi(format!("Read body failed: {e}")));
                } else if status == 503 || status == 429 || status == 502 {
                    last_error = Some(AppError::FoodApi(format!(
                        "Server error (status {status}), retrying..."
                    )));
                    continue;
                } else {
                    return Err(AppError::FoodApi(format!(
                        "Unexpected status {status} for: {url}"
                    )));
                }
            }
            Err(ureq::Error::Status(status, _))
                if status == 503 || status == 429 || status == 502 =>
            {
                last_error = Some(AppError::FoodApi(format!(
                    "Server error (status {status}), retrying..."
                )));
                continue;
            }
            Err(e) => {
                return Err(AppError::FoodApi(format!("Request failed: {e}")));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        AppError::FoodApi(format!(
            "Request failed after {max_retries} retries for: {url}"
        ))
    }))
}
