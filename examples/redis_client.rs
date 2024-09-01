use anyhow::Result;
use redis::ConnectionAddr::Tcp;
use redis::{Client, Commands, ConnectionInfo, ProtocolVersion, RedisConnectionInfo};

fn main() -> Result<()> {
    let client = Client::open(ConnectionInfo {
        addr: Tcp("redis-16430.c1.asia-northeast1-1.gce.redns.redis-cloud.com".to_string(), 16430),
        redis: RedisConnectionInfo {
            db: 0,
            username: Some("default".to_string()),
            password: Some("9JRCAjglNSTc4pXWOggLT7BKljwuoSSy".to_string()),
            protocol: ProtocolVersion::RESP3,
        },
    })?;

    let mut connection = client.get_connection()?;

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
    "#)?;

    Ok(())
}