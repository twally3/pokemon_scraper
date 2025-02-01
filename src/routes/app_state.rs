use sqlx::Sqlite;

#[derive(Clone, Debug)]
pub struct AppState {
    pub pool: sqlx::Pool<Sqlite>,
}
