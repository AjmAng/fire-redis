use bytes::Bytes;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use fire_redis::Store;

fn members(count: usize) -> Vec<Bytes> {
    (0..count)
        .map(|i| Bytes::from(format!("member-{i}")))
        .collect()
}

fn bench_sadd_1k_members(c: &mut Criterion) {
    let members = members(1_000);
    c.bench_function("sadd_1k_members", |b| {
        b.iter_batched(
            || Store::new(),
            |store| {
                store.s_add("bench-sadd".to_string(), members.clone());
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_sismember_hit(c: &mut Criterion) {
    let store = Store::new();
    let members = members(10_000);
    store.s_add("bench-membership".to_string(), members);
    let target = Bytes::from("member-7777");

    c.bench_function("sismember_hit", |b| {
        b.iter(|| {
            criterion::black_box(store.s_is_member("bench-membership", &target));
        })
    });
}

fn bench_spop_100_from_10k(c: &mut Criterion) {
    let members = members(10_000);
    c.bench_function("spop_100_from_10k", |b| {
        b.iter_batched(
            || {
                let store = Store::new();
                store.s_add("bench-spop".to_string(), members.clone());
                store
            },
            |store| {
                criterion::black_box(store.s_pop("bench-spop", 100));
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    store_set_benches,
    bench_sadd_1k_members,
    bench_sismember_hit,
    bench_spop_100_from_10k
);
criterion_main!(store_set_benches);

