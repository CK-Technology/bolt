use bolt::networking::quic::{QUICClient, QUICServer};
use bolt::networking::{NetworkConfig, NetworkDriver, NetworkInterface};
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;
use tokio::runtime::Runtime;

fn test_network_config() -> NetworkConfig {
    NetworkConfig {
        enable_quic: true,
        enable_ebpf: false,
        low_latency: true,
        bandwidth_optimization: true,
        ipv6: false,
        driver: NetworkDriver::QUIC,
    }
}

fn quic_server_setup(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("quic_server_setup");
    group.measurement_time(Duration::from_secs(10));
    group.bench_function("init_server_and_stats", |b| {
        b.to_async(&rt).iter(|| async {
            let server = QUICServer::new(test_network_config()).await.unwrap();
            let stats = server.get_stats().await;
            black_box(stats);
        });
    });
    group.finish();
}

fn quic_container_registration(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let server = rt
        .block_on(async { QUICServer::new(test_network_config()).await })
        .unwrap();

    c.bench_function("register_container_for_quic", |b| {
        b.to_async(&rt).iter(|| async {
            let interface = NetworkInterface {
                container_id: "bench-container".to_string(),
                interface_name: "bolt0".to_string(),
                ip_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
                mac_address: "02:42:7f:00:00:01".to_string(),
                mtu: 1500,
                namespace: "bench".to_string(),
            };
            server
                .register_container("bench-container", &interface)
                .await
                .unwrap();
            let connections = server.get_active_connections().await;
            black_box(connections);
            server
                .unregister_container("bench-container")
                .await
                .unwrap();
        });
    });
}

fn quic_client_setup(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("init_client", |b| {
        b.to_async(&rt).iter(|| async {
            let client = QUICClient::new().await.unwrap();
            black_box(client);
        });
    });
}

criterion_group!(
    benches,
    quic_server_setup,
    quic_container_registration,
    quic_client_setup
);
criterion_main!(benches);
