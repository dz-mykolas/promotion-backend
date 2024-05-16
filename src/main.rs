use axum::{
    routing::{get, post, put}, Extension, Router
};
use geo::Coord;
use std::{collections::HashMap, net::SocketAddr, sync::{Arc, Mutex}};

mod db;
mod location_utils;

mod handlers;

mod database;
use crate::handlers::{accept_friend_request, coin_websocket_handler, get_friend_requests, send_friend_request, websocket_handler};
use tokio::sync::broadcast;
use std::env;
use dotenv::dotenv;

type UserCoords = Arc<Mutex<HashMap<String, Coord<f64>>>>;

#[tokio::main]
async fn main() {
    dotenv().ok();
    let conn = db::setup_db().await;

    match env::var("DATABASE_URL") {
        Ok(database_url) => println!("Database URL: {}", database_url),
        Err(e) => println!("Error: {}", e),
    }

    // let users = users::Entity::find().all(&conn).await.unwrap();
    // println!("Users: {:?}", users);

    // let mock_user = users::ActiveModel {
    //     email: Set("mail@mailhaha.cad".to_owned()),
    //     coins: Set(10),
    //     ..Default::default()
    // };

    // mock_user.save(&conn).await.unwrap();

    // let users = users::Entity::find().all(&conn).await.unwrap();
    // println!("Users: {:?}", users);

    let user_coords: UserCoords = Arc::new(Mutex::new(HashMap::new()));
    let (broadcaster, _receiver) = broadcast::channel::<(String, Coord<f64>)>(10);

    // Define app routes
    let app = Router::new()
        .route("/", get(root_handler))
        .route("/validate_token", get(validate_token_handler))
        .route("/location", get(location_handler))
        .route("/send_friend_request", post(send_friend_request))
        .route("/accept_request/:request_id", put(accept_friend_request))
        .route("/get_friend_requests", get(get_friend_requests))
        .route("/ws_update_coordinates", get(websocket_handler))
        .route("/ws_running_generator", get(coin_websocket_handler))
        .layer(Extension(user_coords))
        .layer(Extension(broadcaster))
        .layer(Extension(conn.clone()));
    
    // Run the server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn root_handler() -> &'static str {
    "Hello, World!"
}

use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;

// Assuming `Claims` is a struct that represents the JWT claims you expect
#[derive(Debug, Deserialize)]
struct Claims {
    sub: String,
    // ... other fields
}

async fn validate_token_handler() -> String {
    let token = "empty";
    validate_token(token).await.unwrap()
}

async fn validate_token(token: &str) -> Result<String, &'static str> {
    let validation = Validation::new(Algorithm::RS256);

    match fetch_jwks_rsa_components().await {
        Ok((n, e)) => {
            let n_str = base64::encode_config(&n, base64::URL_SAFE_NO_PAD);
            let e_str = base64::encode_config(&e, base64::URL_SAFE_NO_PAD);
            let decoding_key = DecodingKey::from_rsa_components(&n_str, &e_str);

            match decode::<Claims>(&token, &decoding_key, &validation) {
                Ok(c) => Ok(c.claims.sub),
                Err(_) => Err("Invalid token"),
            }
        },
        Err(_) => Err("Failed to fetch RSA components"),
    }
}

use jsonwebtoken::errors::Error;
use reqwest;

#[derive(Debug, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
struct Jwk {
    kty: String,
    n: String,
    e: String,
}

async fn fetch_jwks_rsa_components() -> Result<(Vec<u8>, Vec<u8>), Error> {
    let jwks_url = env::var("JWKS_URL")
        .expect("JWKS_URL must be set");

    let res = reqwest::get(jwks_url).await.map_err(|_| Error::from(jsonwebtoken::errors::ErrorKind::InvalidRsaKey))?;

    let jwks: Jwks = res.json().await.map_err(|_| Error::from(jsonwebtoken::errors::ErrorKind::InvalidRsaKey))?;

    if let Some(jwk) = jwks.keys.first() {
        let n_bytes = base64::decode_config(&jwk.n, base64::URL_SAFE_NO_PAD)
            .map_err(|_| Error::from(jsonwebtoken::errors::ErrorKind::InvalidRsaKey))?;
        let e_bytes = base64::decode_config(&jwk.e, base64::URL_SAFE_NO_PAD)
            .map_err(|_| Error::from(jsonwebtoken::errors::ErrorKind::InvalidRsaKey))?;

        Ok((n_bytes, e_bytes))
    } else {
        Err(Error::from(jsonwebtoken::errors::ErrorKind::InvalidRsaKey))
    }
}

// Handler for the /location route
async fn location_handler() -> String {
    let location = location_utils::pick_location_by_day();
    format!("Location of the day: {}, coordinates: {}", location.name, location)
}
