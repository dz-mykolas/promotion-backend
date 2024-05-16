use axum::{extract::{ws::{Message, WebSocket}, Path, Query, WebSocketUpgrade}, response::IntoResponse, Extension, Json};
use chrono::Utc;
use futures::StreamExt;
use geo::{Coord, HaversineDistance, Point};
use reqwest::StatusCode;
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::{database::{friend_requests, friends, users}, validate_token, UserCoords};

#[derive(Deserialize, Serialize)]
pub struct FriendRequestForm {
    sender_id: i32,
    receiver_id: i32,
}

pub async fn send_friend_request(
    Json(payload): Json<FriendRequestForm>,
    Extension(conn): Extension<DatabaseConnection>,
) -> impl IntoResponse {
    let friend_request = friend_requests::ActiveModel {
        sender_id: Set(payload.sender_id),
        receiver_id: Set(payload.receiver_id),
        ..Default::default()
    };

    let insert_result = friend_request.save(&conn).await;

    match insert_result {
        Ok(_) => (StatusCode::CREATED, Json("Request sent")),
        Err(e) => {
            eprintln!("Error sending request: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json("Error sending request"))
        },
    }
}

#[derive(Deserialize, Serialize)]
struct AcceptRequestForm {
    request_id: i32,
}

pub async fn accept_friend_request(
    Path(request_id): Path<i32>,
    Extension(conn): Extension<DatabaseConnection>,
) -> impl IntoResponse {
    // Find the friend request
    let friend_request = friend_requests::Entity::find_by_id(request_id)
        .one(&conn)
        .await;

    match friend_request {
        Ok(Some(request)) => {
            // Check if the request is still pending
            if request.status != "pending" {
                return (StatusCode::BAD_REQUEST, Json("Request is not pending")).into_response();
            }

            // Insert into friends table
            let new_friend = friends::ActiveModel {
                user1_id: Set(request.sender_id.min(request.receiver_id)),
                user2_id: Set(request.sender_id.max(request.receiver_id)),
                ..Default::default()
            };

            let insert_result = new_friend.save(&conn).await;

            match insert_result {
                Ok(_) => {
                    // Update the status of the friend request to 'accepted'
                    let mut active_model = request.into_active_model();
                    active_model.status = Set("accepted".to_string());

                    if let Err(e) = active_model.save(&conn).await {
                        eprintln!("Error updating request status: {:?}", e);
                        return (StatusCode::INTERNAL_SERVER_ERROR, Json("Error updating request status")).into_response();
                    }

                    (StatusCode::OK, Json("Friend request accepted")).into_response()
                },
                Err(e) => {
                    eprintln!("Error creating friend record: {:?}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, Json("Error creating friend record")).into_response()
                }
            }
        },
        Ok(None) => (StatusCode::NOT_FOUND, Json("Request not found")).into_response(),
        Err(e) => {
            eprintln!("Error finding request: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json("Error finding request")).into_response()
        }
    }
}

#[derive(Serialize)]
pub struct FriendRequestResponse {
    pub id: i32,
    pub sender_id: i32,
    pub receiver_id: i32,
    pub created_at: chrono::NaiveDateTime,
    pub status: String,
}

#[derive(Deserialize)]
pub struct GetFriendRequestsQuery {
    pub user_id: i32,
}

pub async fn get_friend_requests(
    Query(query): Query<GetFriendRequestsQuery>,
    Extension(conn): Extension<DatabaseConnection>,
) -> impl IntoResponse {
    // Find friend requests where the user is either the sender or the receiver
    let friend_requests: Vec<friend_requests::Model> = friend_requests::Entity::find()
        .filter(friend_requests::Column::SenderId.eq(query.user_id).or(friend_requests::Column::ReceiverId.eq(query.user_id)))
        .all(&conn)
        .await
        .unwrap_or_else(|_| vec![]);

    let response: Vec<FriendRequestResponse> = friend_requests.into_iter().map(|req| FriendRequestResponse {
        id: req.id,
        sender_id: req.sender_id,
        receiver_id: req.receiver_id,
        created_at: req.created_at,
        status: req.status,
    }).collect();

    (StatusCode::OK, Json(response)).into_response()
}

#[derive(Deserialize)]
struct CoordMessage {
    latitude: f64,
    longitude: f64,
}

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    Extension(user_coords): Extension<UserCoords>,
    Extension(broadcaster): Extension<broadcast::Sender<(String, Coord<f64>)>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, user_coords, broadcaster))
}

async fn handle_socket(
    mut socket: WebSocket,
    user_coords: UserCoords,
    broadcaster: broadcast::Sender<(String, Coord<f64>)>,
) {
    let mut token = String::new();
    if let Some(Ok(Message::Text(t))) = socket.next().await {
        token = t;
    }

    let user_id = match validate_token(&token).await {
        Ok(id) => id,
        Err(_) => {
            let _ = socket.send(Message::Close(None)).await;
            return;
        }
    };

    {
        let mut coords = user_coords.lock().unwrap();
        coords.insert(user_id.clone(), Coord { x: 0.0, y: 0.0 });
    }

    while let Some(Ok(Message::Text(text))) = socket.next().await {
        if let Ok(coord_msg) = serde_json::from_str::<CoordMessage>(&text) {
            let coord = Coord {
                x: coord_msg.latitude,
                y: coord_msg.longitude,
            };

            {
                let mut coords = user_coords.lock().unwrap();
                coords.insert(user_id.clone(), coord);
            }

            let _ = broadcaster.send((user_id.clone(), coord));
        }
    }

    let mut coords = user_coords.lock().unwrap();
    coords.remove(&user_id);
}

pub async fn coin_websocket_handler(
    ws: WebSocketUpgrade,
    Extension(user_coords): Extension<UserCoords>,
    Extension(broadcaster): Extension<broadcast::Sender<(String, Coord<f64>)>>,
    Extension(db): Extension<DatabaseConnection>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_coin_socket(socket, user_coords, broadcaster, db))
}

async fn handle_coin_socket(
    mut socket: WebSocket,
    user_coords: UserCoords,
    broadcaster: broadcast::Sender<(String, Coord<f64>)>,
    db: DatabaseConnection,
) {
    let mut token = String::new();
    if let Some(Ok(Message::Text(t))) = socket.next().await {
        token = t;
    }

    let user_id = match validate_token(&token).await {
        Ok(id) => id,
        Err(_) => {
            let _ = socket.send(Message::Close(None)).await;
            return;
        }
    };

    // Local storage for previous time
    let mut prev_time = Utc::now();
    let mut prev_coord: Option<Coord<f64>> = None;

    while let Some(Ok(Message::Text(text))) = socket.next().await {
        if let Ok(coord_msg) = serde_json::from_str::<CoordMessage>(&text) {
            let new_coord = Coord {
                x: coord_msg.latitude,
                y: coord_msg.longitude,
            };
            let now = Utc::now();

            if let Some(last_coord) = prev_coord {
                let point1 = Point::new(last_coord.x, last_coord.y);
                let point2 = Point::new(new_coord.x, new_coord.y);
                let distance = point1.haversine_distance(&point2) / 1000.0; // in km
                let time_diff = (now - prev_time).num_seconds() as f64 / 3600.0; // in hours
                let speed = distance / time_diff;

                if speed >= 3.0 && speed <= 15.0 {
                    let mut multiplier = 1.0;

                    let other_users = {
                        let coords = user_coords.lock().unwrap();
                        coords.clone()
                    };

                    for (other_id, other_coord) in other_users.iter() {
                        if other_id != &user_id {
                            let point1 = Point::new(new_coord.x, new_coord.y);
                            let point2 = Point::new(other_coord.x, other_coord.y);
                            let distance_to_other = point1.haversine_distance(&point2) / 1000.0; // in km
                            if distance_to_other <= 0.05 { // within 50 meters
                                multiplier = 1.5;
                                break;
                            }
                        }
                    }

                    let coins = calculate_coins(speed) * multiplier;
                    award_coins(&db, user_id.clone(), coins).await;
                }
            }

            {
                let mut coords = user_coords.lock().unwrap();
                coords.insert(user_id.clone(), new_coord);
            }

            prev_coord = Some(new_coord);
            prev_time = now;

            let _ = broadcaster.send((user_id.clone(), new_coord));
        }
    }
}

fn calculate_coins(speed: f64) -> f64 {
    // Implement the coin calculation logic based on speed
    speed * 10.0 // Example: 10 coins per km/h
}

async fn award_coins(
    db: &DatabaseConnection,
    user_id: String,
    coins: f64
) -> impl IntoResponse {
    use sea_orm::ColumnTrait;

    let result = users::Entity::find()
        .filter(users::Column::Email.eq(user_id.clone()))
        .one(db)
        .await;

    match result {
        Ok(Some(user)) => {
            let mut user_active_model: users::ActiveModel = user.into();
            if let ActiveValue::Set(current_coins) = user_active_model.coins {
                user_active_model.coins = Set(current_coins + coins as i32);
            } else {
                // Handle the case where the coins field is not set (e.g., if it's a default value)
                user_active_model.coins = Set(coins as i32);
            }
            let update_result = user_active_model.update(db).await;

            match update_result {
                Ok(_) => (StatusCode::OK, "Coins awarded"),
                Err(e) => {
                    eprintln!("Error updating coins: {:?}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, "Error updating coins")
                },
            }
        },
        Ok(None) => (StatusCode::NOT_FOUND, "User not found"),
        Err(e) => {
            eprintln!("Error finding user: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Error finding user")
        },
    }
}
