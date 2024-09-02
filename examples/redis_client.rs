use anyhow::Result;
use deadpool_redis::Runtime;
use redis::ConnectionAddr::Tcp;
use redis::{AsyncCommands, Client, ConnectionInfo, ProtocolVersion, RedisConnectionInfo};

#[tokio::main]
async fn main() -> Result<()> {
    let config = deadpool_redis::Config::from_connection_info(ConnectionInfo {
        addr: Tcp("10.37.1.132".to_string(), 6379),
        redis: RedisConnectionInfo {
            db: 0,
            username: None,
            password: Some("123456".to_string()),
            protocol: ProtocolVersion::RESP3,
        },
    });
    let pool = config.create_pool(Some(Runtime::Tokio1))?;

    let config1 = deadpool_redis::cluster::Config::from_urls(vec![]);
    let pool1 = config1.create_pool(Some(Runtime::Tokio1))?;


    let mut connection = pool.get().await?;

    let x: String = connection.get("ab").await?;
    println!("{}", x);
    connection.set("json", r#"
{
    "comment": "json with comment", // This is a comment
    "array": [1, true /* another comment */, null,
        {
            "text block": `
            # Title

            > text block in json

            | key | value |
            | --- | ----- |
            | int | i32   |
            `
        }
    ],
    "object": {
        "number": 1.23e4
    }
}
    "#).await?;



    Ok(())
}