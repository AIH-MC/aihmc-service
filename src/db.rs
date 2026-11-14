use serde_json::Value;
use sqlx::{
    MySql, Row,
    mysql::{MySqlArguments, MySqlRow},
    query::Query,
};

use crate::state::AppState;

fn bind(sql: &'static str, arguments: &[Value]) -> Query<'static, MySql, MySqlArguments> {
    let mut query = sqlx::query(sql);
    for argument in arguments {
        query = match argument {
            Value::Null => query.bind(None::<String>),
            Value::Number(number) => query.bind(number.as_i64().unwrap_or_default()),
            Value::Bool(value) => query.bind(*value),
            Value::String(value) => query.bind(value.clone()),
            _ => unreachable!("SQL arguments must be scalars"),
        };
    }
    query
}

pub async fn query(state: &AppState, sql: &'static str, arguments: &[Value]) -> Vec<MySqlRow> {
    match bind(sql, arguments).fetch_all(&state.pool).await {
        Ok(rows) => rows,
        Err(error) => {
            tracing::error!(%error, "database read failed");
            Vec::new()
        }
    }
}

pub async fn execute(state: &AppState, sql: &'static str, arguments: &[Value]) -> bool {
    match bind(sql, arguments).execute(&state.pool).await {
        Ok(_) => true,
        Err(error) => {
            tracing::error!(%error, "database write failed");
            false
        }
    }
}

pub fn text(row: &MySqlRow, column: &str) -> Option<String> {
    row.get::<Option<String>, _>(column)
}
