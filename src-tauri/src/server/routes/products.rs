use std::sync::Arc;
use axum::{extract::{Path, Query, State}, http::StatusCode, routing::{get, put, delete}, Json, Router};
use serde::{Deserialize, Serialize};
use crate::server::{AppState, error::AppError};
use crate::server::db::models::{ProductRow, deserialize_tags_from_input, serialize_tags_to_json_string};

#[derive(Deserialize)]
pub struct ProfileIdFilter {
    pub profile_id: Option<String>,
}

#[derive(Deserialize)]
pub struct ProductCreate {
    pub name: String,
    #[serde(rename = "type")]
    pub product_type: String,
    pub description: Option<String>,
    pub url: Option<String>,
    pub price: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_tags_from_input")]
    pub tags: Option<String>,
    pub profile_id: Option<String>,
}

#[derive(Deserialize)]
pub struct ProductUpdate {
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub product_type: Option<String>,
    pub description: Option<String>,
    pub url: Option<String>,
    pub price: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_tags_from_input")]
    pub tags: Option<String>,
    pub profile_id: Option<String>,
}

#[derive(Serialize)]
pub struct ProductResponse {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub product_type: String,
    pub description: Option<String>,
    pub url: Option<String>,
    pub price: Option<f64>,
    #[serde(serialize_with = "serialize_tags_to_json_string")]
    pub tags: Option<String>,
    pub profile_id: Option<String>,
    pub created_at: Option<String>,
}

impl From<ProductRow> for ProductResponse {
    fn from(r: ProductRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            product_type: r.product_type,
            description: r.description,
            url: r.url,
            price: r.price,
            tags: r.tags,
            profile_id: r.profile_id,
            created_at: r.created_at.map(|dt| dt.to_string()),
        }
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/products", get(list_products).post(create_product))
        .route("/api/products/{product_id}", put(update_product).delete(delete_product))
}

async fn list_products(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ProfileIdFilter>,
) -> Result<Json<Vec<ProductResponse>>, AppError> {
    let rows = if let Some(pid) = params.profile_id {
        sqlx::query_as::<_, ProductRow>(
            "SELECT id, name, type, description, url, price, tags, profile_id, created_at
             FROM products WHERE profile_id = ? ORDER BY created_at DESC"
        ).bind(pid).fetch_all(&state.db).await?
    } else {
        sqlx::query_as::<_, ProductRow>(
            "SELECT id, name, type, description, url, price, tags, profile_id, created_at
             FROM products ORDER BY created_at DESC"
        ).fetch_all(&state.db).await?
    };
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn create_product(
    State(state): State<Arc<AppState>>,
    Json(data): Json<ProductCreate>,
) -> Result<(StatusCode, Json<ProductResponse>), AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO products (id, name, type, description, url, price, tags, profile_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
    )
        .bind(&id)
        .bind(&data.name)
        .bind(&data.product_type)
        .bind(&data.description)
        .bind(&data.url)
        .bind(data.price)
        .bind(&data.tags)
        .bind(&data.profile_id)
        .execute(&state.db).await?;

    let row = sqlx::query_as::<_, ProductRow>(
        "SELECT id, name, type, description, url, price, tags, profile_id, created_at FROM products WHERE id = ?"
    ).bind(&id).fetch_one(&state.db).await?;

    Ok((StatusCode::CREATED, Json(row.into())))
}

async fn update_product(
    State(state): State<Arc<AppState>>,
    Path(product_id): Path<String>,
    Json(data): Json<ProductUpdate>,
) -> Result<Json<ProductResponse>, AppError> {
    let existing = sqlx::query_as::<_, ProductRow>(
        "SELECT id, name, type, description, url, price, tags, profile_id, created_at FROM products WHERE id = ?"
    ).bind(&product_id).fetch_optional(&state.db).await?;

    let row = existing.ok_or_else(|| AppError::NotFound("Product not found".into()))?;

    let name = data.name.unwrap_or(row.name);
    let product_type = data.product_type.unwrap_or(row.product_type);
    let description = data.description.or(row.description);
    let url = data.url.or(row.url);
    let price = data.price.or(row.price);
    let tags = data.tags.or(row.tags);
    let profile_id = data.profile_id.or(row.profile_id);

    sqlx::query(
        "UPDATE products SET name=?, type=?, description=?, url=?, price=?, tags=?, profile_id=? WHERE id=?"
    )
        .bind(&name).bind(&product_type).bind(&description)
        .bind(&url).bind(price).bind(&tags).bind(&profile_id)
        .bind(&product_id)
        .execute(&state.db).await?;

    let updated = sqlx::query_as::<_, ProductRow>(
        "SELECT id, name, type, description, url, price, tags, profile_id, created_at FROM products WHERE id = ?"
    ).bind(&product_id).fetch_one(&state.db).await?;

    Ok(Json(updated.into()))
}

async fn delete_product(
    State(state): State<Arc<AppState>>,
    Path(product_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM products WHERE id = ?"
    ).bind(&product_id).fetch_optional(&state.db).await?;

    if existing.is_none() {
        return Err(AppError::NotFound("Product not found".into()));
    }

    // Check for non-archived campaigns using this product
    let active_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM campaigns WHERE product_id = ? AND status != 'archived'"
    ).bind(&product_id).fetch_one(&state.db).await?;

    if active_count.0 > 0 {
        return Err(AppError::Conflict(format!(
            "Cannot delete product with {} active campaign(s). Archive or delete them first.",
            active_count.0
        )));
    }

    // Cascade delete archived campaigns and their data
    let archived_campaigns: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM campaigns WHERE product_id = ? AND status = 'archived'"
    ).bind(&product_id).fetch_all(&state.db).await?;

    for (cid,) in &archived_campaigns {
        sqlx::query("DELETE FROM metric_snapshots WHERE post_id IN (SELECT id FROM posts WHERE campaign_id = ?)")
            .bind(cid).execute(&state.db).await?;
        sqlx::query("DELETE FROM posts WHERE campaign_id = ?")
            .bind(cid).execute(&state.db).await?;
        sqlx::query("DELETE FROM ai_analyses WHERE campaign_id = ?")
            .bind(cid).execute(&state.db).await?;
    }
    sqlx::query("DELETE FROM campaigns WHERE product_id = ? AND status = 'archived'")
        .bind(&product_id).execute(&state.db).await?;

    sqlx::query("DELETE FROM products WHERE id = ?")
        .bind(&product_id).execute(&state.db).await?;

    Ok(Json(serde_json::json!({"message": "Product deleted", "id": product_id})))
}
