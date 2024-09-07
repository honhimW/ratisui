use anyhow::Result;
use deadpool_redis::Runtime;
use redis::ConnectionAddr::Tcp;
use redis::{AsyncCommands, Client, ConnectionInfo, ProtocolVersion, RedisConnectionInfo};

#[tokio::main]
async fn main() -> Result<()> {
    let config = deadpool_redis::Config::from_connection_info(ConnectionInfo {
        addr: Tcp("redis-16430.c1.asia-northeast1-1.gce.redns.redis-cloud.com".to_string(), 16430),
        redis: RedisConnectionInfo {
            db: 0,
            username: Some(String::from("default")),
            password: Some("9JRCAjglNSTc4pXWOggLT7BKljwuoSSy".to_string()),
            protocol: ProtocolVersion::RESP3,
        },
    });
    let pool = config.create_pool(Some(Runtime::Tokio1))?;

    let mut connection = pool.get().await?;

    connection.set("xml", r#"
<Object>
    <Hello foo="bar">World</Hello>
    <Gender>yes</Gender>
    <Root>false</Root>
    <Number>1</Number>
    <!-- this is comment -->
</Object>
    "#).await?;
//     connection.set("json", r#"
// {
//     "comment": "json with comment", // This is a comment
//     "array": [1, true /* another comment */, null,
//         {
//             "text block": `
//             # Title
//
//             > text block in json
//
//             | key | value |
//             | --- | ----- |
//             | int | i32   |
//             `
//         }
//     ],
//     "object": {
//         "number": 1.23e4
//     }
// }
//     "#).await?;



    Ok(())
}