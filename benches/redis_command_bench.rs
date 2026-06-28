use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use redis::{Commands, Connection};
use std::sync::atomic::{AtomicU64, Ordering};

const DEFAULT_URL: &str = "redis://127.0.0.1:6379/0";
const PREFIX: &str = "criterion:redis-bench";
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn redis_url() -> String {
    std::env::var("REDIS_BENCH_URL").unwrap_or_else(|_| DEFAULT_URL.to_string())
}

fn open_connection() -> Connection {
    let client = redis::Client::open(redis_url()).expect("invalid REDIS_BENCH_URL");
    client
        .get_connection()
        .expect("failed to connect to REDIS_BENCH_URL")
}

fn next_key(tag: &str) -> String {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    format!("{PREFIX}:{tag}:{id}")
}

fn prefill_list(conn: &mut Connection, key: &str, len: usize) {
    let mut pipe = redis::pipe();
    pipe.atomic().cmd("DEL").arg(key);
    for i in 0..len {
        pipe.cmd("RPUSH").arg(key).arg(format!("v{i}"));
    }
    let _: () = pipe.query(conn).expect("prefill list failed");
}

fn prefill_set(conn: &mut Connection, key: &str, len: usize) {
    let mut pipe = redis::pipe();
    pipe.atomic().cmd("DEL").arg(key);
    for i in 0..len {
        pipe.cmd("SADD").arg(key).arg(format!("m{i}"));
    }
    let _: () = pipe.query(conn).expect("prefill set failed");
}

fn bench_ping(c: &mut Criterion) {
    let mut conn = open_connection();
    c.bench_function("redis/ping", |b| {
        b.iter(|| {
            let pong: String = redis::cmd("PING").query(&mut conn).expect("PING failed");
            criterion::black_box(pong);
        })
    });
}

fn bench_set(c: &mut Criterion) {
    let mut conn = open_connection();
    c.bench_function("redis/set", |b| {
        b.iter_batched(
            || next_key("set"),
            |key| {
                let _: () = conn.set(&key, "v").expect("SET failed");
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_get(c: &mut Criterion) {
    c.bench_function("redis/get", |b| {
        b.iter_batched(
            || {
                let mut conn = open_connection();
                let key = next_key("get");
                let _: () = conn.set(&key, "v").expect("setup SET failed");
                (conn, key)
            },
            |(mut conn, key)| {
                let value: Option<String> = conn.get(key).expect("GET failed");
                criterion::black_box(value);
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_incr(c: &mut Criterion) {
    c.bench_function("redis/incr", |b| {
        b.iter_batched(
            || {
                let mut conn = open_connection();
                let key = next_key("incr");
                let _: () = conn.set(&key, 0).expect("setup SET failed");
                (conn, key)
            },
            |(mut conn, key)| {
                let value: isize = conn.incr(key, 1).expect("INCR failed");
                criterion::black_box(value);
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_lpush(c: &mut Criterion) {
    let mut conn = open_connection();
    c.bench_function("redis/lpush", |b| {
        b.iter_batched(
            || next_key("lpush"),
            |key| {
                let _: usize = conn.lpush(key, "v").expect("LPUSH failed");
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_lpop(c: &mut Criterion) {
    c.bench_function("redis/lpop", |b| {
        b.iter_batched(
            || {
                let mut conn = open_connection();
                let key = next_key("lpop");
                prefill_list(&mut conn, &key, 128);
                (conn, key)
            },
            |(mut conn, key)| {
                let value: Option<String> = redis::cmd("LPOP")
                    .arg(key)
                    .query(&mut conn)
                    .expect("LPOP failed");
                criterion::black_box(value);
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_sadd(c: &mut Criterion) {
    let mut conn = open_connection();
    c.bench_function("redis/sadd", |b| {
        b.iter_batched(
            || {
                let key = next_key("sadd");
                let member = format!("m{}", NEXT_ID.fetch_add(1, Ordering::Relaxed));
                (key, member)
            },
            |(key, member)| {
                let _: usize = conn.sadd(key, member).expect("SADD failed");
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_hset(c: &mut Criterion) {
    let mut conn = open_connection();
    c.bench_function("redis/hset", |b| {
        b.iter_batched(
            || {
                let key = next_key("hset");
                let field = format!("f{}", NEXT_ID.fetch_add(1, Ordering::Relaxed));
                (key, field)
            },
            |(key, field)| {
                let _: usize = conn.hset(key, field, "v").expect("HSET failed");
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_spop(c: &mut Criterion) {
    c.bench_function("redis/spop", |b| {
        b.iter_batched(
            || {
                let mut conn = open_connection();
                let key = next_key("spop");
                prefill_set(&mut conn, &key, 128);
                (conn, key)
            },
            |(mut conn, key)| {
                let value: Option<String> = redis::cmd("SPOP")
                    .arg(key)
                    .query(&mut conn)
                    .expect("SPOP failed");
                criterion::black_box(value);
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_zadd(c: &mut Criterion) {
    let mut conn = open_connection();
    c.bench_function("redis/zadd", |b| {
        b.iter_batched(
            || {
                let key = next_key("zadd");
                let member = format!("m{}", NEXT_ID.fetch_add(1, Ordering::Relaxed));
                (key, member)
            },
            |(key, member)| {
                let _: usize = redis::cmd("ZADD")
                    .arg(key)
                    .arg(1.0)
                    .arg(member)
                    .query(&mut conn)
                    .expect("ZADD failed");
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_mset_10(c: &mut Criterion) {
    let mut conn = open_connection();
    c.bench_function("redis/mset_10", |b| {
        b.iter_batched(
            || {
                (0..10)
                    .map(|i| (next_key("mset"), format!("v{i}")))
                    .collect::<Vec<_>>()
            },
            |pairs| {
                let mut cmd = redis::cmd("MSET");
                for (k, v) in pairs {
                    cmd.arg(k).arg(v);
                }
                let _: () = cmd.query(&mut conn).expect("MSET failed");
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_lrange(c: &mut Criterion, window: usize) {
    let name = format!("redis/lrange_{window}");
    c.bench_function(&name, |b| {
        b.iter_batched(
            || {
                let mut conn = open_connection();
                let key = next_key("lrange");
                prefill_list(&mut conn, &key, window);
                (conn, key)
            },
            |(mut conn, key)| {
                let stop = window as isize - 1;
                let value: Vec<String> = redis::cmd("LRANGE")
                    .arg(key)
                    .arg(0)
                    .arg(stop)
                    .query(&mut conn)
                    .expect("LRANGE failed");
                criterion::black_box(value);
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_lrange_100(c: &mut Criterion) {
    bench_lrange(c, 100);
}

fn bench_lrange_300(c: &mut Criterion) {
    bench_lrange(c, 300);
}

fn bench_lrange_500(c: &mut Criterion) {
    bench_lrange(c, 500);
}

fn bench_lrange_600(c: &mut Criterion) {
    bench_lrange(c, 600);
}

criterion_group!(
    name = redis_command_benches;
    config = Criterion::default().sample_size(20);
    targets =
        bench_ping,
        bench_set,
        bench_get,
        bench_incr,
        bench_lpush,
        bench_lpop,
        bench_sadd,
        bench_hset,
        bench_spop,
        bench_zadd,
        bench_mset_10,
        bench_lrange_100,
        bench_lrange_300,
        bench_lrange_500,
        bench_lrange_600
);
criterion_main!(redis_command_benches);



