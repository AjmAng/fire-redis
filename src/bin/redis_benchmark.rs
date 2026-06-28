use clap::Parser;
use redis::Commands;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

const DEFAULT_TESTS: &str =
    "ping,set,get,incr,lpush,lpop,sadd,hset,spop,zadd,mset,lrange_100,lrange_300,lrange_500,lrange_600";

#[derive(Debug, Clone, Parser)]
#[command(name = "redis-benchmark")]
#[command(about = "Standalone Redis-style benchmark binary for Redis-compatible servers")]
struct Args {
    /// Redis URL, for example redis://127.0.0.1:6379/0
    #[arg(short = 'u', long, default_value = "redis://127.0.0.1:6379/0")]
    url: String,

    /// Total requests per test
    #[arg(short = 'n', long, default_value_t = 100_000)]
    requests: usize,

    /// Concurrent clients per test
    #[arg(short = 'c', long, default_value_t = 50)]
    clients: usize,

    /// Data size in bytes for string payloads
    #[arg(short = 'd', long, default_value_t = 3)]
    data_size: usize,

    /// Number of key slots
    #[arg(short = 'r', long, default_value_t = 10_000)]
    keyspace: usize,

    /// Comma-separated tests to run
    #[arg(short = 't', long, default_value = DEFAULT_TESTS)]
    tests: String,

    /// Number of key-value pairs for MSET
    #[arg(long, default_value_t = 10)]
    mset_size: usize,

    /// Key prefix for benchmark data
    #[arg(long, default_value = "redis-bench")]
    prefix: String,

    /// Print CSV instead of table
    #[arg(long, default_value_t = false)]
    csv: bool,

    /// Delete benchmark keys before each test
    #[arg(long, default_value_t = false)]
    clear: bool,
}

#[derive(Debug, Clone, Copy)]
enum Op {
    Ping,
    Set,
    Get,
    Incr,
    Lpush,
    Lpop,
    Sadd,
    Hset,
    Spop,
    Zadd,
    Mset,
    Lrange(usize),
}

impl Op {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "ping" => Some(Self::Ping),
            "set" => Some(Self::Set),
            "get" => Some(Self::Get),
            "incr" => Some(Self::Incr),
            "lpush" => Some(Self::Lpush),
            "lpop" => Some(Self::Lpop),
            "sadd" => Some(Self::Sadd),
            "hset" => Some(Self::Hset),
            "spop" => Some(Self::Spop),
            "zadd" => Some(Self::Zadd),
            "mset" => Some(Self::Mset),
            "lrange_100" => Some(Self::Lrange(100)),
            "lrange_300" => Some(Self::Lrange(300)),
            "lrange_500" => Some(Self::Lrange(500)),
            "lrange_600" => Some(Self::Lrange(600)),
            _ => None,
        }
    }

    fn name(self) -> String {
        match self {
            Self::Ping => "PING".to_string(),
            Self::Set => "SET".to_string(),
            Self::Get => "GET".to_string(),
            Self::Incr => "INCR".to_string(),
            Self::Lpush => "LPUSH".to_string(),
            Self::Lpop => "LPOP".to_string(),
            Self::Sadd => "SADD".to_string(),
            Self::Hset => "HSET".to_string(),
            Self::Spop => "SPOP".to_string(),
            Self::Zadd => "ZADD".to_string(),
            Self::Mset => "MSET".to_string(),
            Self::Lrange(window) => format!("LRANGE_{window}"),
        }
    }

    fn key_tag(self) -> String {
        self.name().to_lowercase()
    }
}

#[derive(Default)]
struct WorkerResult {
    latencies_ms: Vec<f64>,
    done: usize,
    errors: usize,
}

struct BenchResult {
    op: String,
    requests: usize,
    clients: usize,
    done: usize,
    errors: usize,
    duration_s: f64,
    rps: f64,
    p50_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
}

fn log_step(message: impl AsRef<str>) {
    eprintln!("[bench] {}", message.as_ref());
}

fn payload(size: usize) -> String {
    "x".repeat(size.max(1))
}

fn split_work(total: usize, workers: usize) -> Vec<usize> {
    let base = total / workers;
    let extra = total % workers;
    (0..workers)
        .map(|i| if i < extra { base + 1 } else { base })
        .collect()
}

fn quantile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((p / 100.0) * ((sorted.len() - 1) as f64)).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn bench_key(prefix: &str, op: Op, key_id: usize) -> String {
    format!("{prefix}:{}:{key_id}", op.key_tag())
}

fn open_connection(url: &str) -> redis::Connection {
    let client = redis::Client::open(url).expect("invalid redis url");
    client.get_connection().expect("failed to connect to redis")
}

fn clear_prefix(url: &str, prefix: &str) {
    let mut conn = open_connection(url);
    let pattern = format!("{prefix}:*");
    let mut cursor = 0_u64;

    loop {
        let (next, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(&pattern)
            .arg("COUNT")
            .arg(1000)
            .query(&mut conn)
            .expect("SCAN failed");

        if !keys.is_empty() {
            let _: usize = redis::cmd("DEL")
                .arg(keys)
                .query(&mut conn)
                .expect("DEL failed");
        }

        cursor = next;
        if cursor == 0 {
            break;
        }
    }
}

fn prefill(url: &str, op: Op, args: &Args, value: &str) {
    let op_name = op.name();
    let started = Instant::now();
    let mut conn = open_connection(url);
    let keyspace = args.keyspace.max(1);

    log_step(format!(
        "prefill start: op={} keyspace={}",
        op_name, keyspace
    ));

    match op {
        Op::Get => {
            for i in 0..keyspace {
                let key = bench_key(&args.prefix, op, i);
                let _: () = conn.set(key, value).expect("prefill SET failed");
            }
        }
        Op::Lpop => {
            for i in 0..keyspace {
                let key = bench_key(&args.prefix, op, i);
                let mut pipe = redis::pipe();
                // Keep prefill transaction-free for Redis-compatible servers without MULTI/EXEC.
                pipe.cmd("DEL").arg(&key).ignore();
                for n in 0..128 {
                    pipe.cmd("RPUSH").arg(&key).arg(format!("v{n}")).ignore();
                }
                let _: () = pipe.query(&mut conn).expect("prefill list failed");
            }
        }
        Op::Spop => {
            for i in 0..keyspace {
                let key = bench_key(&args.prefix, op, i);
                let mut pipe = redis::pipe();
                pipe.cmd("DEL").arg(&key).ignore();
                for n in 0..128 {
                    pipe.cmd("SADD").arg(&key).arg(format!("m{n}")).ignore();
                }
                let _: () = pipe.query(&mut conn).expect("prefill set failed");
            }
        }
        Op::Lrange(window) => {
            for i in 0..keyspace {
                let key = bench_key(&args.prefix, op, i);
                let mut pipe = redis::pipe();
                pipe.cmd("DEL").arg(&key).ignore();
                for n in 0..window {
                    pipe.cmd("RPUSH").arg(&key).arg(format!("v{n}")).ignore();
                }
                let _: () = pipe.query(&mut conn).expect("prefill lrange list failed");
            }
        }
        _ => {}
    }

    log_step(format!(
        "prefill done: op={} elapsed={:.3}s",
        op_name,
        started.elapsed().as_secs_f64()
    ));
}

fn run_op(op: Op, args: &Args) -> BenchResult {
    let op_name = op.name();
    log_step(format!(
        "test start: op={} requests={} clients={} keyspace={}",
        op_name,
        args.requests,
        args.clients.max(1),
        args.keyspace.max(1)
    ));

    let workers = args.clients.max(1);
    let keyspace = args.keyspace.max(1);
    let data = payload(args.data_size);
    prefill(&args.url, op, args, &data);

    let chunks = split_work(args.requests, workers);
    let (tx, rx) = mpsc::channel::<WorkerResult>();
    let started = Instant::now();

    for (worker_id, chunk) in chunks.into_iter().enumerate() {
        if chunk == 0 {
            continue;
        }

        let tx = tx.clone();
        let url = args.url.clone();
        let prefix = args.prefix.clone();
        let data = data.clone();
        let mset_size = args.mset_size.max(1);

        thread::spawn(move || {
            let mut conn = open_connection(&url);
            let mut out = WorkerResult {
                latencies_ms: Vec::with_capacity(chunk),
                done: 0,
                errors: 0,
            };

            for idx in 0..chunk {
                let mixed = idx.wrapping_mul(1_103_515_245).wrapping_add(worker_id * 12_345);
                let key_id = mixed % keyspace;
                let key = bench_key(&prefix, op, key_id);

                let t0 = Instant::now();
                let result = match op {
                    Op::Ping => redis::cmd("PING").query::<String>(&mut conn).map(|_| ()),
                    Op::Set => conn.set::<_, _, ()>(key, &data),
                    Op::Get => conn.get::<_, Option<String>>(key).map(|_| ()),
                    Op::Incr => conn.incr::<_, _, isize>(key, 1).map(|_| ()),
                    Op::Lpush => conn.lpush::<_, _, usize>(key, &data).map(|_| ()),
                    Op::Lpop => redis::cmd("LPOP").arg(key).query::<Option<String>>(&mut conn).map(|_| ()),
                    Op::Sadd => {
                        let member = format!("m{worker_id}:{idx}");
                        conn.sadd::<_, _, usize>(key, member).map(|_| ())
                    }
                    Op::Hset => {
                        let field = format!("f{worker_id}:{idx}");
                        conn.hset::<_, _, _, usize>(key, field, &data).map(|_| ())
                    }
                    Op::Spop => redis::cmd("SPOP").arg(key).query::<Option<String>>(&mut conn).map(|_| ()),
                    Op::Zadd => {
                        let member = format!("m{worker_id}:{idx}");
                        redis::cmd("ZADD")
                            .arg(key)
                            .arg(1.0)
                            .arg(member)
                            .query::<usize>(&mut conn)
                            .map(|_| ())
                    }
                    Op::Mset => {
                        let mut cmd = redis::cmd("MSET");
                        for j in 0..mset_size {
                            let sub_key_id = (key_id + j) % keyspace;
                            let sub_key = bench_key(&prefix, op, sub_key_id);
                            cmd.arg(sub_key).arg(&data);
                        }
                        cmd.query::<()>(&mut conn).map(|_| ())
                    }
                    Op::Lrange(window) => redis::cmd("LRANGE")
                        .arg(key)
                        .arg(0)
                        .arg(window as isize - 1)
                        .query::<Vec<String>>(&mut conn)
                        .map(|_| ()),
                };

                let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
                out.latencies_ms.push(elapsed_ms);

                if result.is_ok() {
                    out.done += 1;
                } else {
                    out.errors += 1;
                }
            }

            let _ = tx.send(out);
        });
    }

    drop(tx);

    let mut latencies = Vec::with_capacity(args.requests);
    let mut done = 0usize;
    let mut errors = 0usize;

    for part in rx {
        done += part.done;
        errors += part.errors;
        latencies.extend(part.latencies_ms);
    }

    let duration_s = started.elapsed().as_secs_f64();
    latencies.sort_by(f64::total_cmp);

    let result = BenchResult {
        op: op_name,
        requests: args.requests,
        clients: workers,
        done,
        errors,
        duration_s,
        rps: if duration_s > 0.0 {
            (done as f64) / duration_s
        } else {
            0.0
        },
        p50_ms: quantile(&latencies, 50.0),
        p95_ms: quantile(&latencies, 95.0),
        p99_ms: quantile(&latencies, 99.0),
    };

    log_step(format!(
        "test done: op={} done={} errors={} rps={:.0} elapsed={:.3}s",
        result.op, result.done, result.errors, result.rps, result.duration_s
    ));

    result
}

fn parse_tests(input: &str) -> Result<Vec<Op>, String> {
    let mut ops = Vec::new();
    for raw in input.split(',') {
        let name = raw.trim().to_lowercase();
        if name.is_empty() {
            continue;
        }
        match Op::parse(&name) {
            Some(op) => ops.push(op),
            None => return Err(format!("unsupported test: {name}")),
        }
    }

    if ops.is_empty() {
        return Err("no tests selected".to_string());
    }
    Ok(ops)
}

fn print_table(results: &[BenchResult]) {
    println!(
        "{:<12} {:>10} {:>10} {:>10} {:>12} {:>10} {:>10} {:>10}",
        "test", "rps", "p50(ms)", "p95(ms)", "p99(ms)", "errors", "done", "secs"
    );
    for r in results {
        println!(
            "{:<12} {:>10.0} {:>10.3} {:>10.3} {:>12.3} {:>10} {:>10} {:>10.3}",
            r.op, r.rps, r.p50_ms, r.p95_ms, r.p99_ms, r.errors, r.done, r.duration_s
        );
    }
}

fn print_csv(results: &[BenchResult]) {
    println!("test,rps,p50_ms,p95_ms,p99_ms,errors,done,requests,clients,duration_s");
    for r in results {
        println!(
            "{},{:.3},{:.6},{:.6},{:.6},{},{},{},{},{:.6}",
            r.op, r.rps, r.p50_ms, r.p95_ms, r.p99_ms, r.errors, r.done, r.requests, r.clients, r.duration_s
        );
    }
}

fn main() {
    let args = Args::parse();
    let tests = match parse_tests(&args.tests) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(2);
        }
    };

    if args.clear {
        log_step(format!("initial cleanup: prefix={}", args.prefix));
        clear_prefix(&args.url, &args.prefix);
    }

    log_step(format!(
        "run start: url={} tests={} requests_per_test={} clients={} csv={}",
        args.url,
        tests.len(),
        args.requests,
        args.clients.max(1),
        args.csv
    ));

    let mut results = Vec::with_capacity(tests.len());
    for (idx, op) in tests.iter().copied().enumerate() {
        log_step(format!(
            "progress: test {}/{} ({})",
            idx + 1,
            tests.len(),
            op.name()
        ));

        if args.clear {
            log_step(format!("cleanup before test: prefix={}", args.prefix));
            clear_prefix(&args.url, &args.prefix);
        }
        results.push(run_op(op, &args));
    }

    if args.csv {
        print_csv(&results);
    } else {
        print_table(&results);
    }

    log_step("run complete");
}


