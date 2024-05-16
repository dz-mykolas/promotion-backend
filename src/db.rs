use sea_orm::prelude::*;
use sea_orm::{Database, DatabaseConnection};
use std::env;

pub async fn setup_db() -> DatabaseConnection {
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    let conn: DatabaseConnection = Database::connect(&database_url).await.unwrap();
    
    conn
}

// Allow this function to have dead code
#[allow(dead_code)]
pub async fn find_tables(conn: &DatabaseConnection) -> Vec<String> {
    let raw_sql = "SELECT table_name 
        FROM information_schema.tables
        WHERE table_schema = 'public'
        AND table_type = 'BASE TABLE'
        AND table_catalog = 'promotion_db'
    ;";

    // let tables = sql_query(raw_sql).fetch_all(conn).await.unwrap();
    // let tables: Vec<String> = tables.iter().map(|table| table.get("table_name")).collect();
    /* mock tables */
    let tables = vec!["users".to_string(), "promotions".to_string(), "locations".to_string()];

    tables
}