use std::env;
use tokio_postgres::NoTls;
use deadpool_postgres::{Config, ManagerConfig, Pool, RecyclingMethod, Runtime};

pub async fn pg_connection() -> Pool {

  let pg_db_name = env::var("PG_DBNAME").unwrap();
  let pg_db_uname = env::var("PG_USERNAME").unwrap();
  let pg_db_password = env::var("PG_PASSWORD").unwrap();

  let mut cfg = Config::new();
  cfg.dbname = Some(pg_db_name);
  cfg.user = Some(pg_db_uname);
  cfg.password = Some(pg_db_password);
  cfg.manager = Some(ManagerConfig { recycling_method: RecyclingMethod::Fast });
  let pool: Pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls).unwrap();
  pool
}