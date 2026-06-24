use clap::Parser;
use fire_redis::{RespCodec, Value};
use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(name = "redis-cli")]
#[command(about = "Redis command line interface")]
struct Args {
    //
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Server port
    #[arg(long, default_value_t = 6379)]
    port: u16,

    /// Command to execute (if not provided, enters interactive mode)
    #[arg(value_name = "COMMAND")]
    cmd: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let addr = format!("{}:{}", args.host, args.port);

    let stream = TcpStream::connect(&addr).await?;
    info!("Connected to redis server at {}", addr);

    let mut framed = Framed::new(stream, RespCodec);

    if args.cmd.is_empty() {
        run_interactive(&mut framed).await?;
    } else {
        let resp = parse_args_to_resp(&args.cmd);
        framed.send(resp).await?;

        if let Some(Ok(response)) = framed.next().await {
            print_response(&response);
        }
    }

    Ok(())
}

/// interactive REPL
async fn run_interactive(
    framed: &mut Framed<TcpStream, RespCodec>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("redis-cli connected. Type 'QUIT' to exit.");
    println!("Ready:");

    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<String> = line.split_whitespace().map(|s| s.to_string()).collect();

        if parts.is_empty() {
            continue;
        }

        if parts[0].to_ascii_uppercase() == "QUIT" {
            println!("Goodbye!");
            break;
        }

        let resp = parse_args_to_resp(&parts);
        if let Err(e) = framed.send(resp).await {
            error!("Failed to send command: {}", e);
            continue;
        }

        match framed.next().await {
            Some(Ok(response)) => print_response(&response),
            Some(Err(e)) => error!("Error: {}", e),
            None => {
                error!("Server disconnected");
                break;
            }
        }
    }

    Ok(())
}

/// transform Vec<String> to RESP Array of Bulk Strings
fn parse_args_to_resp(args: &[String]) -> Value {
    let bulk_strings: Vec<Value> = args
        .iter()
        .map(|s| Value::BulkString(Some(bytes::Bytes::from(s.clone()))))
        .collect();

    Value::Array(Some(bulk_strings))
}

/// beautifully print RESP response
fn print_response(value: &Value) {
    match value {
        Value::SimpleString(s) => println!("\"{}\"", s),
        Value::Integer(i) => println!("(integer) {}", i),
        Value::BulkString(None) | Value::Null => println!("(nil)"),
        Value::BulkString(Some(b)) => match std::str::from_utf8(b) {
            Ok(s) => println!("\"{}\"", s),
            Err(_) => println!("{:?}", b),
        },
        Value::Error(e) => println!("(error) {}", e),
        Value::Array(Some(arr)) => {
            if arr.is_empty() {
                println!("(empty array)");
            } else {
                for (i, item) in arr.iter().enumerate() {
                    print!("{}) ", i + 1);
                    print_response_inline(item);
                }
            }
        }
        Value::Array(None) => println!("(nil)"),
    }
}

fn print_response_inline(value: &Value) {
    match value {
        Value::BulkString(Some(b)) => {
            if let Ok(s) = std::str::from_utf8(b) {
                println!("\"{}\"", s);
            } else {
                println!("[binary {:?}]", b.len());
            }
        }
        _ => print_response(value),
    }
}
